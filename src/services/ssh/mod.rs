//! SSH tunnel support for database connections.
//!
//! Opens a local TCP listener on `127.0.0.1:<random>` and forwards each
//! accepted connection to a remote host:port over an SSH `direct-tcpip`
//! channel. The local bound port is used by sqlx as if it were the real
//! database server.
//!
//! Authentication is key-based:
//! - private key file (optional passphrase)
//! - SSH agent (via `SSH_AUTH_SOCK`, when available on the platform)

mod config;
mod tunnel;

pub use config::{SshAuth, SshConfig};
pub use tunnel::SshTunnel;
