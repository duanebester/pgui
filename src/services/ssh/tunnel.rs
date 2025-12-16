//! SSH tunnel implementation using system ssh binary.
//!
//! This follows Zed's approach of using the system's `ssh` binary rather than
//! a library like `ssh2`. Benefits include:
//! - Leverages user's existing SSH config (~/.ssh/config)
//! - Uses system's ssh-agent automatically
//! - Works with ProxyJump/bastion hosts
//! - Uses ControlMaster on Unix for connection multiplexing

use super::types::{SshAuthMethod, SshTunnelConfig};
use anyhow::{Context, Result};
use futures::StreamExt;
use smol::io::{AsyncBufReadExt, BufReader};
use smol::net::TcpListener;
use smol::process::{Child, Command, Stdio};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// An active SSH tunnel using the system ssh binary with -L port forwarding.
pub struct SshTunnel {
    config: SshTunnelConfig,
    local_port: u16,
    process: Child,
    #[cfg(unix)]
    _temp_dir: tempfile::TempDir,
}

#[allow(dead_code)]
impl SshTunnel {
    /// Start a new SSH tunnel using the system ssh binary.
    ///
    /// This spawns an `ssh -L` process that forwards a local port to the remote host.
    pub async fn start(config: SshTunnelConfig) -> Result<Self> {
        // Find an available local port if not specified
        let local_port = if config.local_bind_port == 0 {
            Self::find_available_port(&config.local_bind_host).await?
        } else {
            config.local_bind_port
        };

        // Create temp directory for control socket and askpass script
        #[cfg(unix)]
        let temp_dir = tempfile::Builder::new()
            .prefix("pgui-ssh-")
            .tempdir()
            .context("Failed to create temp directory")?;

        #[cfg(unix)]
        let socket_path = temp_dir.path().join("ssh.sock");

        // Build the port forwarding spec: local_port:remote_host:remote_port
        let forward_spec = format!(
            "{}:{}:{}:{}",
            config.local_bind_host, local_port, config.remote_host, config.remote_port
        );

        let mut cmd = Command::new("ssh");

        // Kill the ssh process when this handle is dropped
        cmd.kill_on_drop(true);

        // Don't need stdin for the tunnel, capture stdout/stderr for debugging
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
        // In production, you might want to be more careful here
        cmd.args(["-o", "StrictHostKeyChecking=accept-new"]);

        // Connection keep-alive settings
        cmd.args(["-o", "ServerAliveInterval=15"]);
        cmd.args(["-o", "ServerAliveCountMax=3"]);

        #[cfg(unix)]
        {
            // Use ControlMaster for connection reuse (Unix only)
            cmd.args(["-o", "ControlMaster=auto"]);
            cmd.args(["-o", "ControlPersist=60"]);
            cmd.arg("-o")
                .arg(format!("ControlPath={}", socket_path.display()));
        }

        // Set port if non-default
        if config.ssh_port != 22 {
            cmd.arg("-p").arg(config.ssh_port.to_string());
        }

        // Handle authentication
        match &config.auth_method {
            SshAuthMethod::Agent => {
                // Default behavior - ssh will use ssh-agent automatically
            }
            SshAuthMethod::Password(password) => {
                // Set up askpass for password authentication
                #[cfg(unix)]
                Self::setup_askpass(&mut cmd, &temp_dir, password)?;

                #[cfg(not(unix))]
                {
                    // On Windows, we'll rely on the user having configured their SSH
                    // or we could use plink with password, but that's more complex
                    tracing::warn!(
                        "Password authentication via askpass not supported on Windows, \
                        falling back to default SSH auth"
                    );
                    let _ = password; // Suppress unused warning
                }
            }
            SshAuthMethod::PublicKey {
                private_key_path,
                passphrase,
            } => {
                cmd.arg("-i").arg(private_key_path);
                // If there's a passphrase, set up askpass
                if let Some(pass) = passphrase {
                    #[cfg(unix)]
                    Self::setup_askpass(&mut cmd, &temp_dir, pass)?;
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

        // Wait briefly for the tunnel to establish
        smol::Timer::after(Duration::from_millis(500)).await;

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
        let mut retries = 5;
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
            _temp_dir: temp_dir,
        })
    }

    /// Set up SSH_ASKPASS for password/passphrase authentication (Unix only)
    #[cfg(unix)]
    fn setup_askpass(cmd: &mut Command, temp_dir: &tempfile::TempDir, secret: &str) -> Result<()> {
        // Create a simple askpass script that echoes the password
        // This is similar to Zed's approach but simplified
        let askpass_path = temp_dir.path().join("askpass.sh");

        // Write the script - it just echoes the password
        // In a more sophisticated setup, this could communicate via Unix socket
        let script = format!("#!/bin/sh\necho '{}'\n", secret.replace('\'', "'\"'\"'"));
        std::fs::write(&askpass_path, script).context("Failed to write askpass script")?;

        // Make it executable
        let mut perms = std::fs::metadata(&askpass_path)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&askpass_path, perms)?;

        // Set environment variables for SSH to use our askpass
        cmd.env("SSH_ASKPASS", &askpass_path);
        cmd.env("SSH_ASKPASS_REQUIRE", "force");
        // Need to detach from terminal for askpass to work
        cmd.env("DISPLAY", ":0");

        Ok(())
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
        // Create temp directory for askpass if needed
        #[cfg(unix)]
        let temp_dir = tempfile::Builder::new()
            .prefix("pgui-ssh-test-")
            .tempdir()
            .context("Failed to create temp directory")?;

        let mut cmd = Command::new("ssh");

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Just try to run 'true' or 'exit 0' to test connection
        cmd.args(["-o", "BatchMode=yes"]); // Don't prompt for password in test
        cmd.args(["-o", "ConnectTimeout=10"]);
        cmd.args(["-o", "StrictHostKeyChecking=accept-new"]);

        if config.ssh_port != 22 {
            cmd.arg("-p").arg(config.ssh_port.to_string());
        }

        // Handle authentication for test
        match &config.auth_method {
            SshAuthMethod::Agent => {}
            SshAuthMethod::Password(password) => {
                #[cfg(unix)]
                Self::setup_askpass(&mut cmd, &temp_dir, password)?;
                // Remove BatchMode for password auth
                cmd.args(["-o", "BatchMode=no"]);
            }
            SshAuthMethod::PublicKey {
                private_key_path,
                passphrase,
            } => {
                cmd.arg("-i").arg(private_key_path);
                if let Some(pass) = passphrase {
                    #[cfg(unix)]
                    Self::setup_askpass(&mut cmd, &temp_dir, pass)?;
                    cmd.args(["-o", "BatchMode=no"]);
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

impl Drop for SshTunnel {
    fn drop(&mut self) {
        // Ensure process is killed when tunnel is dropped
        let _ = self.process.kill();
    }
}
