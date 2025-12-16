//! SSH tunnel configuration types.

use serde::{Deserialize, Serialize};

/// Authentication method for SSH connections
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SshAuthMethod {
    /// Use ssh-agent or system default authentication (recommended)
    Agent,
    /// Password authentication (will use SSH_ASKPASS)
    Password(String),
    /// Public key file with optional passphrase
    PublicKey {
        private_key_path: String,
        passphrase: Option<String>,
    },
}

impl Default for SshAuthMethod {
    fn default() -> Self {
        SshAuthMethod::Agent
    }
}

/// Configuration for an SSH tunnel using system ssh binary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshTunnelConfig {
    /// SSH server hostname
    pub ssh_host: String,
    /// SSH server port (default: 22)
    pub ssh_port: u16,
    /// SSH username
    pub ssh_user: String,
    /// Authentication method
    pub auth_method: SshAuthMethod,
    /// Remote host to tunnel to (as seen from SSH server)
    pub remote_host: String,
    /// Remote port to tunnel to
    pub remote_port: u16,
    /// Local bind address (default: 127.0.0.1)
    pub local_bind_host: String,
    /// Local port to bind (0 for auto-assign)
    pub local_bind_port: u16,
    /// Additional SSH arguments (e.g., from user's config)
    pub extra_args: Vec<String>,
}

#[allow(dead_code)]
impl SshTunnelConfig {
    /// Create a new SSH tunnel configuration using ssh-agent authentication
    pub fn with_agent(
        ssh_host: impl Into<String>,
        ssh_port: u16,
        ssh_user: impl Into<String>,
        remote_host: impl Into<String>,
        remote_port: u16,
    ) -> Self {
        Self {
            ssh_host: ssh_host.into(),
            ssh_port,
            ssh_user: ssh_user.into(),
            auth_method: SshAuthMethod::Agent,
            remote_host: remote_host.into(),
            remote_port,
            local_bind_host: "127.0.0.1".to_string(),
            local_bind_port: 0,
            extra_args: Vec::new(),
        }
    }

    /// Create a new SSH tunnel configuration with password authentication
    pub fn with_password(
        ssh_host: impl Into<String>,
        ssh_port: u16,
        ssh_user: impl Into<String>,
        password: impl Into<String>,
        remote_host: impl Into<String>,
        remote_port: u16,
    ) -> Self {
        Self {
            ssh_host: ssh_host.into(),
            ssh_port,
            ssh_user: ssh_user.into(),
            auth_method: SshAuthMethod::Password(password.into()),
            remote_host: remote_host.into(),
            remote_port,
            local_bind_host: "127.0.0.1".to_string(),
            local_bind_port: 0,
            extra_args: Vec::new(),
        }
    }

    /// Create a new SSH tunnel configuration with public key authentication
    pub fn with_public_key(
        ssh_host: impl Into<String>,
        ssh_port: u16,
        ssh_user: impl Into<String>,
        private_key_path: impl Into<String>,
        passphrase: Option<String>,
        remote_host: impl Into<String>,
        remote_port: u16,
    ) -> Self {
        Self {
            ssh_host: ssh_host.into(),
            ssh_port,
            ssh_user: ssh_user.into(),
            auth_method: SshAuthMethod::PublicKey {
                private_key_path: private_key_path.into(),
                passphrase,
            },
            remote_host: remote_host.into(),
            remote_port,
            local_bind_host: "127.0.0.1".to_string(),
            local_bind_port: 0,
            extra_args: Vec::new(),
        }
    }

    /// Set the local bind port (0 for auto-assign)
    pub fn with_local_port(mut self, port: u16) -> Self {
        self.local_bind_port = port;
        self
    }

    /// Add extra SSH arguments
    pub fn with_extra_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Build the SSH URL (user@host)
    pub fn ssh_url(&self) -> String {
        if self.ssh_user.is_empty() {
            self.ssh_host.clone()
        } else {
            format!("{}@{}", self.ssh_user, self.ssh_host)
        }
    }
}
