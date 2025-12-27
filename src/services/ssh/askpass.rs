//! Secure password delivery for SSH via Unix socket.
//!
//! This module implements a secure mechanism for providing passwords to SSH
//! without ever writing them to the filesystem. Instead of creating a script
//! that echoes the password (which leaves secrets on disk), we:
//!
//! 1. Create a Unix socket with restrictive permissions (0600)
//! 2. Write a minimal script that connects to the socket using the pgui binary
//! 3. Serve the password over the socket when SSH invokes askpass
//!
//! Security properties:
//! - Password never written to filesystem
//! - Socket has restrictive permissions (0600)
//! - Script contains only socket path, no secrets
//! - Temp directory cleaned up on drop
//!
//! # Usage in binaries
//!
//! Call `handle_askpass_mode()` at the very start of main() before any other
//! initialization. If the `--askpass` flag is present, it will handle the
//! password delivery and exit. Otherwise, it returns and your program continues.
//!
//! ```ignore
//! fn main() {
//!     // Must be called first, before logging or other init
//!     pgui::ssh::handle_askpass_mode();
//!
//!     // Rest of your program...
//! }
//! ```

use anyhow::{Context, Result};
use futures::FutureExt;
use smol::io::AsyncWriteExt;
use smol::net::unix::UnixListener;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Check for `--askpass` mode and handle it if present.
///
/// This function should be called at the very start of `main()`, before any
/// other initialization (logging, GUI, etc.). If the `--askpass <socket_path>`
/// arguments are present, this function will:
///
/// 1. Connect to the Unix socket at the given path
/// 2. Read the password from the socket
/// 3. Print it to stdout (for SSH to read)
/// 4. Exit the process with code 0 (success) or 1 (error)
///
/// If `--askpass` is not present, this function returns normally and your
/// program continues with its regular execution.
///
/// # Example
///
/// ```ignore
/// fn main() {
///     pgui::ssh::handle_askpass_mode();
///
///     // Normal program initialization continues here...
///     init_logging();
///     run_app();
/// }
/// ```
pub fn handle_askpass_mode() {
    let args: Vec<String> = std::env::args().collect();

    if let Some(pos) = args.iter().position(|a| a == "--askpass") {
        if let Some(socket_path) = args.get(pos + 1) {
            match handle_askpass(socket_path) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("askpass error: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("--askpass requires a socket path argument");
            std::process::exit(1);
        }
    }
    // If --askpass not present, return normally
}

/// Connect to the askpass socket and print the password to stdout.
#[cfg(unix)]
fn handle_askpass(socket_path: &str) -> std::io::Result<()> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path)?;
    let mut password = String::new();
    stream.read_to_string(&mut password)?;

    // Print to stdout (SSH reads from askpass stdout)
    print!("{}", password);
    std::io::stdout().flush()?;

    Ok(())
}

#[cfg(not(unix))]
fn handle_askpass(_socket_path: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "askpass mode is only supported on Unix systems",
    ))
}

/// Serves passwords over a Unix socket for SSH_ASKPASS.
///
/// When SSH needs a password, it invokes the askpass script. Our script
/// connects to this proxy's Unix socket, and we send the password directly
/// through the socket - never touching the filesystem.
pub struct AskpassProxy {
    listener: UnixListener,
    script_path: PathBuf,
    #[allow(dead_code)]
    socket_path: PathBuf,
    _temp_dir: TempDir,
}

impl AskpassProxy {
    /// Create a new askpass proxy ready to serve a password.
    ///
    /// This creates:
    /// - A temporary directory with 0700 permissions
    /// - A Unix socket with 0600 permissions
    /// - A minimal shell script that connects to the socket
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::with_prefix("pgui-ssh-")?;
        let socket_path = temp_dir.path().join("askpass.sock");
        let script_path = temp_dir.path().join("askpass.sh");

        // Set directory permissions to 0700 (owner rwx only)
        #[cfg(unix)]
        {
            let mut perms = std::fs::metadata(temp_dir.path())?.permissions();
            perms.set_mode(0o700);
            std::fs::set_permissions(temp_dir.path(), perms)?;
        }

        // Create the Unix socket listener (synchronous in smol)
        let listener =
            UnixListener::bind(&socket_path).context("Failed to create askpass socket")?;

        // Set socket permissions to 0600 (owner rw only)
        #[cfg(unix)]
        {
            let mut perms = std::fs::metadata(&socket_path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&socket_path, perms)?;
        }

        // Get the current executable path for self-as-askpass
        let current_exe =
            std::env::current_exe().context("Failed to get current executable path")?;

        // Shell-escape the paths to handle special characters
        let escaped_exe = shell_escape(&current_exe.to_string_lossy());
        let escaped_socket = shell_escape(&socket_path.to_string_lossy());

        // Write a script that invokes our own binary with --askpass flag.
        // This is more reliable than depending on `nc` being available.
        // Falls back to nc if the binary doesn't support --askpass yet.
        let script = format!(
            r#"#!/bin/sh
# Secure askpass proxy - password delivered via Unix socket
# Try using pgui binary first, fall back to nc
if {} --askpass {} 2>/dev/null; then
    exit 0
elif command -v nc >/dev/null 2>&1; then
    nc -U {}
else
    echo "Error: Neither pgui --askpass nor nc available" >&2
    exit 1
fi
"#,
            escaped_exe, escaped_socket, escaped_socket
        );
        std::fs::write(&script_path, &script)?;

        // Make script executable (0700)
        #[cfg(unix)]
        {
            let mut perms = std::fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o700);
            std::fs::set_permissions(&script_path, perms)?;
        }

        tracing::debug!(
            "Created askpass proxy: socket={}, script={}",
            socket_path.display(),
            script_path.display()
        );

        Ok(Self {
            listener,
            script_path,
            socket_path,
            _temp_dir: temp_dir,
        })
    }

    /// Path to the askpass script (set as SSH_ASKPASS)
    pub fn script_path(&self) -> &PathBuf {
        &self.script_path
    }

    /// Serve a password once when SSH requests it.
    ///
    /// This waits for the askpass script to connect via Unix socket,
    /// then sends the password and closes the connection.
    ///
    /// Note: This should be called in a background task since it blocks
    /// until SSH invokes askpass.
    pub async fn serve_password(&self, password: &str) -> Result<()> {
        let (mut stream, _addr) = self
            .listener
            .accept()
            .await
            .context("Failed to accept askpass connection")?;

        // Send the password followed by newline (SSH expects this)
        stream
            .write_all(password.as_bytes())
            .await
            .context("Failed to write password to socket")?;
        stream
            .write_all(b"\n")
            .await
            .context("Failed to write newline to socket")?;
        stream
            .flush()
            .await
            .context("Failed to flush password to socket")?;

        // Close our end to signal EOF
        drop(stream);

        tracing::debug!("Served password via askpass proxy");
        Ok(())
    }

    /// Serve a password with a timeout.
    ///
    /// Returns Ok(true) if password was served, Ok(false) if timed out.
    pub async fn serve_password_with_timeout(
        &self,
        password: &str,
        timeout: std::time::Duration,
    ) -> Result<bool> {
        let serve_future = self.serve_password(password);
        let timeout_future = smol::Timer::after(timeout);

        futures::select! {
            result = Box::pin(serve_future).fuse() => {
                result?;
                Ok(true)
            }
            _ = Box::pin(timeout_future).fuse() => {
                tracing::debug!("Askpass timeout - SSH may not have needed the password");
                Ok(false)
            }
        }
    }
}

/// Shell-escape a string for safe inclusion in a shell script.
/// This handles special characters that could cause issues.
fn shell_escape(s: &str) -> String {
    // If the string contains only safe characters, return as-is with quotes
    // Otherwise, use single quotes and escape any single quotes within
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '/' || c == '.' || c == '-' || c == '_')
    {
        format!("'{}'", s)
    } else {
        // Replace single quotes with '\'' (end quote, escaped quote, start quote)
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

impl Drop for AskpassProxy {
    fn drop(&mut self) {
        // TempDir handles cleanup, but log it for debugging
        tracing::debug!("Cleaning up askpass proxy");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_askpass_proxy_creation() {
        smol::block_on(async {
            let proxy = AskpassProxy::new().await.unwrap();

            // Verify script exists and is executable
            assert!(proxy.script_path().exists());

            // Verify script contains socket path, not password
            let script_content = std::fs::read_to_string(proxy.script_path()).unwrap();
            assert!(script_content.contains("--askpass"));
            assert!(!script_content.contains("secret"));
        });
    }

    #[test]
    fn test_askpass_password_delivery() {
        smol::block_on(async {
            let proxy = AskpassProxy::new().await.unwrap();
            let socket_path = proxy.socket_path.clone();
            let password = "test_secret_password";

            // Spawn a task to serve the password
            let serve_task = smol::spawn(async move {
                proxy
                    .serve_password_with_timeout(password, Duration::from_secs(5))
                    .await
            });

            // Give the listener a moment to start
            smol::Timer::after(Duration::from_millis(50)).await;

            // Connect as the askpass script would
            let mut stream = smol::net::unix::UnixStream::connect(&socket_path)
                .await
                .unwrap();

            // Read the password
            use smol::io::AsyncReadExt;
            let mut received = String::new();
            stream.read_to_string(&mut received).await.unwrap();

            assert_eq!(received.trim(), password);

            // Verify serve completed successfully
            let result = serve_task.await.unwrap();
            assert!(result);
        });
    }

    #[test]
    fn test_askpass_timeout() {
        smol::block_on(async {
            let proxy = AskpassProxy::new().await.unwrap();

            // Serve with a short timeout - no one connects
            let result = proxy
                .serve_password_with_timeout("password", Duration::from_millis(100))
                .await
                .unwrap();

            // Should return false (timed out), not error
            assert!(!result);
        });
    }

    #[test]
    fn test_shell_escape() {
        assert_eq!(shell_escape("simple"), "'simple'");
        assert_eq!(shell_escape("/path/to/file"), "'/path/to/file'");
        assert_eq!(shell_escape("path with spaces"), "'path with spaces'");
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }
}
