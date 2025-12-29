//! SSH tunnel configuration types.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// A host that can be either an IP address or a hostname
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Host {
    Ip(IpAddr),
    Name(String),
}

impl Host {
    /// Format for use in SSH port forwarding (-L) specs
    /// IPv6 addresses need brackets: [::1]
    pub fn to_bracketed_string(&self) -> String {
        match self {
            Host::Ip(IpAddr::V6(ip)) => format!("[{}]", ip),
            Host::Ip(IpAddr::V4(ip)) => ip.to_string(),
            Host::Name(name) => name.clone(),
        }
    }
}

impl std::fmt::Display for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Host::Ip(ip) => write!(f, "{}", ip),
            Host::Name(name) => write!(f, "{}", name),
        }
    }
}

impl<S: AsRef<str>> From<S> for Host {
    fn from(s: S) -> Self {
        let s = s.as_ref();
        // Try parsing as IP first
        if let Ok(ip) = s.parse::<IpAddr>() {
            Host::Ip(ip)
        } else {
            Host::Name(s.to_string())
        }
    }
}

impl Default for Host {
    fn default() -> Self {
        Host::Name(String::new())
    }
}

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
    /// SSH server hostname or IP
    pub ssh_host: Host,
    /// SSH server port (default: 22)
    pub ssh_port: u16,
    /// SSH username
    pub ssh_user: String,
    /// Authentication method
    pub auth_method: SshAuthMethod,
    /// Remote host to tunnel to (as seen from SSH server)
    pub remote_host: Host,
    /// Remote port to tunnel to
    pub remote_port: u16,
    /// Local bind address (default: 127.0.0.1)
    pub local_bind_host: Host,
    /// Local port to bind (0 for auto-assign)
    pub local_bind_port: u16,
    /// Additional SSH arguments (e.g., from user's config)
    pub extra_args: Vec<String>,
}

#[allow(dead_code)]
impl SshTunnelConfig {
    /// Create a new SSH tunnel configuration using ssh-agent authentication
    pub fn with_agent(
        ssh_host: impl Into<Host>,
        ssh_port: u16,
        ssh_user: impl Into<String>,
        remote_host: impl Into<Host>,
        remote_port: u16,
    ) -> Self {
        Self {
            ssh_host: ssh_host.into(),
            ssh_port,
            ssh_user: ssh_user.into(),
            auth_method: SshAuthMethod::Agent,
            remote_host: remote_host.into(),
            remote_port,
            local_bind_host: Host::from("127.0.0.1"),
            local_bind_port: 0,
            extra_args: Vec::new(),
        }
    }

    /// Create a new SSH tunnel configuration with password authentication
    pub fn with_password(
        ssh_host: impl Into<Host>,
        ssh_port: u16,
        ssh_user: impl Into<String>,
        password: impl Into<String>,
        remote_host: impl Into<Host>,
        remote_port: u16,
    ) -> Self {
        Self {
            ssh_host: ssh_host.into(),
            ssh_port,
            ssh_user: ssh_user.into(),
            auth_method: SshAuthMethod::Password(password.into()),
            remote_host: remote_host.into(),
            remote_port,
            local_bind_host: Host::from("127.0.0.1"),
            local_bind_port: 0,
            extra_args: Vec::new(),
        }
    }

    /// Create a new SSH tunnel configuration with public key authentication
    pub fn with_public_key(
        ssh_host: impl Into<Host>,
        ssh_port: u16,
        ssh_user: impl Into<String>,
        private_key_path: impl Into<String>,
        passphrase: Option<String>,
        remote_host: impl Into<Host>,
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
            local_bind_host: Host::from("127.0.0.1"),
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
            self.ssh_host.to_string()
        } else {
            format!("{}@{}", self.ssh_user, self.ssh_host)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_host_from_ipv4_string() {
        let host = Host::from("192.168.1.1");
        assert_eq!(host, Host::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert_eq!(host.to_string(), "192.168.1.1");
        assert_eq!(host.to_bracketed_string(), "192.168.1.1");
    }

    #[test]
    fn test_host_from_ipv6_string() {
        let host = Host::from("::1");
        assert_eq!(host, Host::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert_eq!(host.to_string(), "::1");
        assert_eq!(host.to_bracketed_string(), "[::1]");
    }

    #[test]
    fn test_host_from_ipv6_full() {
        let host = Host::from("2001:db8::1");
        match host {
            Host::Ip(IpAddr::V6(_)) => {}
            _ => panic!("Expected IPv6 address"),
        }
        assert_eq!(host.to_bracketed_string(), "[2001:db8::1]");
    }

    #[test]
    fn test_host_from_hostname() {
        let host = Host::from("db.example.com");
        assert_eq!(host, Host::Name("db.example.com".to_string()));
        assert_eq!(host.to_string(), "db.example.com");
        assert_eq!(host.to_bracketed_string(), "db.example.com");
    }

    #[test]
    fn test_host_from_localhost() {
        let host = Host::from("localhost");
        assert_eq!(host, Host::Name("localhost".to_string()));
        assert_eq!(host.to_bracketed_string(), "localhost");
    }

    #[test]
    fn test_host_default() {
        let host = Host::default();
        assert_eq!(host, Host::Name(String::new()));
    }

    #[test]
    fn test_ipv6_ssh_forward_spec_format() {
        // This is the main use case: formatting for SSH -L option
        let local_host = Host::from("::1");
        let remote_host = Host::from("db.internal");
        let forward_spec = format!(
            "{}:{}:{}:{}",
            local_host.to_bracketed_string(),
            5432,
            remote_host.to_bracketed_string(),
            5432
        );
        assert_eq!(forward_spec, "[::1]:5432:db.internal:5432");
    }

    #[test]
    fn test_ipv4_ssh_forward_spec_format() {
        let local_host = Host::from("127.0.0.1");
        let remote_host = Host::from("db.internal");
        let forward_spec = format!(
            "{}:{}:{}:{}",
            local_host.to_bracketed_string(),
            5432,
            remote_host.to_bracketed_string(),
            5432
        );
        assert_eq!(forward_spec, "127.0.0.1:5432:db.internal:5432");
    }

    #[test]
    fn test_serde_ipv4() {
        let host = Host::from("192.168.1.1");
        let json = serde_json::to_string(&host).unwrap();
        assert_eq!(json, "\"192.168.1.1\"");

        let parsed: Host = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, host);
    }

    #[test]
    fn test_serde_ipv6() {
        let host = Host::from("::1");
        let json = serde_json::to_string(&host).unwrap();
        assert_eq!(json, "\"::1\"");

        let parsed: Host = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, host);
    }

    #[test]
    fn test_serde_hostname() {
        let host = Host::from("db.example.com");
        let json = serde_json::to_string(&host).unwrap();
        assert_eq!(json, "\"db.example.com\"");

        let parsed: Host = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, host);
    }
}
