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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_port_is_22() {
        assert_eq!(SshConfig::default().port, 22);
    }

    #[test]
    fn default_auth_is_agent() {
        assert!(matches!(SshConfig::default().auth, SshAuth::Agent));
    }

    #[test]
    fn ssh_auth_partial_eq() {
        // Two KeyFile values with the same path are equal; differing paths
        // are not. Important because the form uses PartialEq comparisons
        // when deciding whether to re-prompt for a passphrase.
        let a = SshAuth::KeyFile {
            path: "/a".to_string(),
        };
        let b = SshAuth::KeyFile {
            path: "/a".to_string(),
        };
        let c = SshAuth::KeyFile {
            path: "/b".to_string(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, SshAuth::Agent);
    }

    #[test]
    fn ssh_auth_serde_tagging() {
        let json = serde_json::to_string(&SshAuth::Agent).unwrap();
        assert_eq!(json, r#"{"type":"agent"}"#);
        let json = serde_json::to_string(&SshAuth::KeyFile {
            path: "/x".to_string(),
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"key_file","path":"/x"}"#);
    }
}
