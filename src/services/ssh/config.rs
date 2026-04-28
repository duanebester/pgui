//! SSH connection configuration.

use serde::{Deserialize, Serialize};

/// How to authenticate to the SSH server.
///
/// Only key-based authentication is supported; password auth for SSH itself
/// is intentionally out of scope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SshAuth {
    /// Authenticate using a private key file. Passphrase is optional and,
    /// when present, is stored in the system keyring (not in this struct).
    KeyFile { path: String },
    /// Authenticate via the running SSH agent (`SSH_AUTH_SOCK`).
    Agent,
}

impl SshAuth {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            SshAuth::KeyFile { .. } => "key_file",
            SshAuth::Agent => "agent",
        }
    }
}

impl Default for SshAuth {
    fn default() -> Self {
        SshAuth::Agent
    }
}

/// SSH tunnel configuration.
///
/// Sensitive values (key passphrase) are not stored here — they are loaded
/// on demand from the keyring at connect time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            username: String::new(),
            auth: SshAuth::default(),
        }
    }
}
