//! SSH tunnel implementation using system ssh binary.
//!
//! This follows Zed's approach of using the system's `ssh` binary rather than
//! a library like `ssh2`. Benefits include:
//! - Leverages user's existing SSH config (~/.ssh/config)
//! - Uses system's ssh-agent automatically
//! - Works with ProxyJump/bastion hosts

use super::askpass::AskpassProxy;
use super::types::{SshAuthMethod, SshTunnelConfig};
use anyhow::{Context, Result};
use futures::StreamExt;
use smol::io::{AsyncBufReadExt, BufReader};
use smol::net::TcpListener;
use smol::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// An active SSH tunnel using the system ssh binary with -L port forwarding.
pub struct SshTunnel {
    config: SshTunnelConfig,
    local_port: u16,
    process: Child,
    /// Holds the askpass proxy alive while the tunnel is running.
    /// Uses Arc so we can share with the password-serving task.
    #[cfg(unix)]
    _askpass_proxy: Option<Arc<AskpassProxy>>,
    /// Cancellation flag to stop askpass tasks when tunnel is shutting down.
    #[cfg(unix)]
    cancelled: Arc<AtomicBool>,
}

/// Maximum attempts to find and bind to an available port.
/// This handles the TOCTOU race condition where another process could grab
/// the port between when we find it and when SSH tries to bind.
const MAX_PORT_RETRY_ATTEMPTS: u32 = 3;

#[allow(dead_code)]
impl SshTunnel {
    /// Start a new SSH tunnel using the system ssh binary.
    ///
    /// This spawns an `ssh -L` process that forwards a local port to the remote host.
    pub async fn start(config: SshTunnelConfig) -> Result<Self> {
        // If user specified a port, use it directly (no retry on port conflicts)
        if config.local_bind_port != 0 {
            return Self::start_with_port(config.clone(), config.local_bind_port).await;
        }

        // Auto port selection: retry if we hit a port race condition
        let mut last_error = None;
        for attempt in 1..=MAX_PORT_RETRY_ATTEMPTS {
            let local_port = Self::find_available_port(&config.local_bind_host).await?;

            match Self::start_with_port(config.clone(), local_port).await {
                Ok(tunnel) => return Ok(tunnel),
                Err(e) => {
                    let error_str = e.to_string();
                    // Check if this looks like a port binding issue
                    if error_str.contains("not listening")
                        || error_str.contains("Address already in use")
                    {
                        tracing::warn!(
                            "Port {} was taken before SSH could bind (attempt {}/{}), retrying with new port",
                            local_port,
                            attempt,
                            MAX_PORT_RETRY_ATTEMPTS
                        );
                        last_error = Some(e);
                        continue;
                    }
                    // Non-port-related error, don't retry
                    return Err(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!(
                "Failed to find available port after {} attempts",
                MAX_PORT_RETRY_ATTEMPTS
            )
        }))
    }

    /// Start a tunnel with a specific local port.
    /// This is the inner implementation used by start().
    async fn start_with_port(config: SshTunnelConfig, local_port: u16) -> Result<Self> {
        // Build the port forwarding spec: local_port:remote_host:remote_port
        let forward_spec = format!(
            "{}:{}:{}:{}",
            config.local_bind_host, local_port, config.remote_host, config.remote_port
        );

        let mut cmd = Command::new("ssh");

        // Kill the ssh process when the Child is dropped. The async-process crate's
        // internal Reaper task will asynchronously wait on the process to prevent
        // zombie processes, so no explicit Drop implementation is needed.
        cmd.kill_on_drop(true);

        // Don't need stdin for the tunnel, capture stdout/stderr for debugging
        // Using Stdio::null() for stdin means SSH has no TTY, which helps askpass work
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Port forwarding - this is the core of our tunnel
        cmd.arg("-L").arg(&forward_spec);

        // Keep connection alive but don't execute a remote command
        cmd.arg("-N");

        // Exit immediately if we can't set up the port forwarding
        cmd.args(["-o", "ExitOnForwardFailure=yes"]);

        // Disable strict host key checking for testing (user can override via extra_args)
        cmd.args(["-o", "StrictHostKeyChecking=accept-new"]);

        // Connection keep-alive settings
        cmd.args(["-o", "ServerAliveInterval=15"]);
        cmd.args(["-o", "ServerAliveCountMax=3"]);

        // Explicitly disable ControlMaster to prevent SSH from backgrounding
        cmd.args(["-o", "ControlMaster=no"]);

        // Set port if non-default
        if config.ssh_port != 22 {
            cmd.arg("-p").arg(config.ssh_port.to_string());
        }

        // Handle authentication - set up askpass proxy if needed
        #[cfg(unix)]
        let askpass_proxy: Option<Arc<AskpassProxy>>;
        #[cfg(unix)]
        let cancelled = Arc::new(AtomicBool::new(false));

        match &config.auth_method {
            SshAuthMethod::Agent => {
                // Default behavior - ssh will use ssh-agent automatically
                #[cfg(unix)]
                {
                    askpass_proxy = None;
                }
            }
            SshAuthMethod::Password(password) => {
                #[cfg(unix)]
                {
                    let proxy = Arc::new(
                        AskpassProxy::new()
                            .await
                            .context("Failed to create askpass proxy")?,
                    );
                    Self::configure_askpass_env(&mut cmd, &proxy);

                    // Spawn background task to serve the password.
                    // The proxy is kept alive by the Arc in the tunnel struct.
                    let proxy_clone = Arc::clone(&proxy);
                    let password = password.clone();
                    let cancelled_clone = Arc::clone(&cancelled);
                    smol::spawn(async move {
                        // Serve password multiple times in case SSH re-prompts
                        // (e.g., for 2FA or retry on typo)
                        for attempt in 0..3 {
                            // Check if tunnel was cancelled before waiting
                            if cancelled_clone.load(Ordering::Relaxed) {
                                tracing::debug!("Askpass task cancelled before attempt {}", attempt + 1);
                                break;
                            }
                            match proxy_clone
                                .serve_password_with_timeout(&password, Duration::from_secs(60))
                                .await
                            {
                                Ok(true) => {
                                    tracing::debug!(
                                        "Served password via askpass (attempt {})",
                                        attempt + 1
                                    );
                                }
                                Ok(false) => {
                                    // Timeout - SSH probably authenticated already or cancelled
                                    if cancelled_clone.load(Ordering::Relaxed) {
                                        tracing::debug!("Askpass task cancelled during timeout");
                                    } else {
                                        tracing::debug!(
                                            "Askpass timeout (attempt {}), SSH may be authenticated",
                                            attempt + 1
                                        );
                                    }
                                    break;
                                }
                                Err(e) => {
                                    tracing::warn!("Askpass proxy error: {}", e);
                                    break;
                                }
                            }
                        }
                    })
                    .detach();

                    // Store the proxy to keep it alive for the tunnel's lifetime
                    askpass_proxy = Some(proxy);
                }

                #[cfg(not(unix))]
                {
                    tracing::warn!("Password authentication via askpass not supported on Windows");
                }
            }
            SshAuthMethod::PublicKey {
                private_key_path,
                passphrase,
            } => {
                cmd.arg("-i").arg(private_key_path);
                // If there's a passphrase, set up askpass
                #[cfg(unix)]
                {
                    if let Some(pass) = passphrase {
                        let proxy = Arc::new(
                            AskpassProxy::new()
                                .await
                                .context("Failed to create askpass proxy for key passphrase")?,
                        );
                        Self::configure_askpass_env(&mut cmd, &proxy);

                        let proxy_clone = Arc::clone(&proxy);
                        let pass = pass.clone();
                        let cancelled_clone = Arc::clone(&cancelled);
                        smol::spawn(async move {
                            // SSH may prompt multiple times for passphrase
                            for attempt in 0..3 {
                                // Check if tunnel was cancelled before waiting
                                if cancelled_clone.load(Ordering::Relaxed) {
                                    tracing::debug!(
                                        "Askpass passphrase task cancelled before attempt {}",
                                        attempt + 1
                                    );
                                    break;
                                }
                                match proxy_clone
                                    .serve_password_with_timeout(&pass, Duration::from_secs(60))
                                    .await
                                {
                                    Ok(true) => {
                                        tracing::debug!(
                                            "Served key passphrase via askpass (attempt {})",
                                            attempt + 1
                                        );
                                    }
                                    Ok(false) => {
                                        // Timeout or cancelled
                                        if cancelled_clone.load(Ordering::Relaxed) {
                                            tracing::debug!(
                                                "Askpass passphrase task cancelled during timeout"
                                            );
                                        } else {
                                            tracing::debug!(
                                                "Askpass timeout for passphrase (attempt {})",
                                                attempt + 1
                                            );
                                        }
                                        break;
                                    }
                                    Err(e) => {
                                        tracing::warn!("Askpass proxy error for passphrase: {}", e);
                                        break;
                                    }
                                }
                            }
                        })
                        .detach();

                        askpass_proxy = Some(proxy);
                    } else {
                        askpass_proxy = None;
                    }
                }
            }
        }

        // Add extra args from user config
        for arg in &config.extra_args {
            cmd.arg(arg);
        }

        // Finally, add the destination
        cmd.arg(config.ssh_url());

        tracing::info!(
            "Starting SSH tunnel: ssh -L {} -N {}",
            forward_spec,
            config.ssh_url()
        );
        tracing::debug!("Full SSH command: {:?}", cmd);

        let mut process = cmd.spawn().context("Failed to spawn ssh process")?;

        // Monitor stderr for connection status
        let stderr = process.stderr.take();
        if let Some(stderr) = stderr {
            let config_clone = config.clone();
            smol::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Some(result) = lines.next().await {
                    match result {
                        Ok(line) => {
                            if line.contains("Permission denied")
                                || line.contains("Authentication failed")
                            {
                                tracing::error!(
                                    "SSH authentication failed for {}: {}",
                                    config_clone.ssh_url(),
                                    line
                                );
                            } else if line.contains("Connection refused")
                                || line.contains("Connection timed out")
                            {
                                tracing::error!("SSH connection error: {}", line);
                            } else {
                                tracing::debug!("SSH stderr: {}", line);
                            }
                        }
                        Err(e) => {
                            tracing::debug!("SSH stderr read error: {}", e);
                            break;
                        }
                    }
                }
            })
            .detach();
        }

        // Wait for the tunnel to establish - give it more time
        smol::Timer::after(Duration::from_millis(1000)).await;

        // Check if process is still running
        if let Ok(Some(status)) = process.try_status() {
            anyhow::bail!(
                "SSH process exited immediately with status: {}. \
                Check SSH credentials and connectivity to {}:{}",
                status,
                config.ssh_host,
                config.ssh_port
            );
        }

        // Try to verify the local port is actually listening
        let verify_addr = format!("{}:{}", config.local_bind_host, local_port);
        let mut retries = 10;
        while retries > 0 {
            match smol::net::TcpStream::connect(&verify_addr).await {
                Ok(_) => {
                    tracing::info!(
                        "SSH tunnel established: {} -> {}:{}",
                        verify_addr,
                        config.remote_host,
                        config.remote_port
                    );
                    break;
                }
                Err(_) if retries > 1 => {
                    smol::Timer::after(Duration::from_millis(200)).await;
                    retries -= 1;
                }
                Err(e) => {
                    // Kill the process since tunnel didn't work
                    let _ = process.kill();
                    anyhow::bail!(
                        "SSH tunnel failed to establish - local port {} not listening: {}",
                        local_port,
                        e
                    );
                }
            }
        }

        Ok(Self {
            config,
            local_port,
            process,
            #[cfg(unix)]
            _askpass_proxy: askpass_proxy,
            #[cfg(unix)]
            cancelled,
        })
    }

    /// Configure SSH environment variables to use askpass proxy
    #[cfg(unix)]
    fn configure_askpass_env(cmd: &mut Command, proxy: &AskpassProxy) {
        cmd.env("SSH_ASKPASS", proxy.script_path());
        // SSH_ASKPASS_REQUIRE=force makes SSH use askpass even without a terminal
        cmd.env("SSH_ASKPASS_REQUIRE", "force");
        // Use existing DISPLAY or fallback - SSH needs some value for askpass to work
        let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
        cmd.env("DISPLAY", display);
    }

    /// Find an available port to bind to
    async fn find_available_port(bind_host: &str) -> Result<u16> {
        let listener = TcpListener::bind(format!("{}:0", bind_host))
            .await
            .context("Failed to find available port")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        Ok(port)
    }

    /// Get the local port the tunnel is listening on.
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    /// Get the local address to connect to (e.g., "127.0.0.1:12345").
    pub fn local_addr(&self) -> String {
        format!("{}:{}", self.config.local_bind_host, self.local_port)
    }

    /// Check if the tunnel process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.process.try_status(), Ok(None))
    }

    /// Shutdown the tunnel gracefully.
    pub async fn shutdown(mut self) {
        tracing::debug!("Shutting down SSH tunnel to {}", self.config.ssh_url());

        // Signal askpass tasks to stop waiting
        #[cfg(unix)]
        self.cancelled.store(true, Ordering::Relaxed);

        // Try graceful termination first
        #[cfg(unix)]
        {
            // Send SIGTERM
            unsafe {
                libc::kill(self.process.id() as i32, libc::SIGTERM);
            }
            // Give it a moment to clean up
            smol::Timer::after(Duration::from_millis(100)).await;
        }

        // Force kill if still running
        if self.is_alive() {
            let _ = self.process.kill();
        }

        // Wait for process to fully exit
        let _ = self.process.status().await;

        tracing::info!("SSH tunnel shutdown complete");
    }

    /// Test SSH connection without starting a full tunnel.
    /// This verifies we can connect and authenticate to the SSH server.
    pub async fn test_ssh_connection(config: &SshTunnelConfig) -> Result<()> {
        let mut cmd = Command::new("ssh");

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        cmd.args(["-o", "ConnectTimeout=10"]);
        cmd.args(["-o", "StrictHostKeyChecking=accept-new"]);

        if config.ssh_port != 22 {
            cmd.arg("-p").arg(config.ssh_port.to_string());
        }

        // Handle authentication
        #[cfg(unix)]
        let _proxy_handle: Option<Arc<AskpassProxy>>;

        match &config.auth_method {
            SshAuthMethod::Agent => {
                cmd.args(["-o", "BatchMode=yes"]);
                #[cfg(unix)]
                {
                    _proxy_handle = None;
                }
            }
            SshAuthMethod::Password(password) => {
                #[cfg(unix)]
                {
                    let proxy = Arc::new(
                        AskpassProxy::new()
                            .await
                            .context("Failed to create askpass proxy for test")?,
                    );
                    Self::configure_askpass_env(&mut cmd, &proxy);

                    let proxy_clone = Arc::clone(&proxy);
                    let password = password.clone();
                    smol::spawn(async move {
                        let _ = proxy_clone
                            .serve_password_with_timeout(&password, Duration::from_secs(30))
                            .await;
                    })
                    .detach();

                    _proxy_handle = Some(proxy);
                }

                #[cfg(not(unix))]
                {
                    tracing::warn!("Password authentication via askpass not supported on Windows");
                    let _ = password;
                }
            }
            SshAuthMethod::PublicKey {
                private_key_path,
                passphrase,
            } => {
                cmd.arg("-i").arg(private_key_path);
                #[cfg(unix)]
                {
                    if let Some(pass) = passphrase {
                        let proxy = Arc::new(
                            AskpassProxy::new()
                                .await
                                .context("Failed to create askpass proxy for key test")?,
                        );
                        Self::configure_askpass_env(&mut cmd, &proxy);

                        let proxy_clone = Arc::clone(&proxy);
                        let pass = pass.clone();
                        smol::spawn(async move {
                            let _ = proxy_clone
                                .serve_password_with_timeout(&pass, Duration::from_secs(30))
                                .await;
                        })
                        .detach();

                        _proxy_handle = Some(proxy);
                    } else {
                        cmd.args(["-o", "BatchMode=yes"]);
                        _proxy_handle = None;
                    }
                }
            }
        }

        cmd.arg(config.ssh_url());
        cmd.arg("exit");
        cmd.arg("0");

        tracing::debug!("Testing SSH connection: {:?}", cmd);

        let output = cmd
            .output()
            .await
            .context("Failed to execute ssh test command")?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "SSH connection test failed (exit {}): {}",
                output.status,
                stderr.trim()
            )
        }
    }
}
