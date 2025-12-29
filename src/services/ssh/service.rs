//! SSH tunnel management service with GPUI integration.
//!
//! This module provides:
//! - `SshService` - Global service for managing SSH tunnels
//! - `TunnelState` - Lifecycle states for UI feedback
//! - Keychain integration for SSH passwords

use super::reconnect::{ExponentialBackoff, ReconnectConfig, is_retriable_error};
use super::tunnel::SshTunnel;
use super::types::{SshAuthMethod, SshTunnelConfig};
use anyhow::{Context, Result};
use async_channel::{Receiver, Sender};
use gpui::Global;
use keyring::Entry;
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a tunnel
pub type TunnelId = Uuid;

/// Keyring service name for SSH passwords
const SSH_KEYRING_SERVICE: &str = "pgui-ssh";

/// Tunnel lifecycle states for UI feedback
#[derive(Debug, Clone, PartialEq)]
pub enum TunnelState {
    /// Initial state, setting up
    Connecting,
    /// Tunnel is active and healthy
    Connected { local_addr: String },
    /// Tunnel dropped, attempting reconnect
    Reconnecting { attempt: u32, max_attempts: u32 },
    /// Tunnel failed, not retrying
    Failed { error: String },
    /// Tunnel was intentionally closed
    #[allow(dead_code)]
    Closed,
}

#[allow(dead_code)]
impl TunnelState {
    /// Returns true if the tunnel is usable for connections
    pub fn is_connected(&self) -> bool {
        matches!(self, TunnelState::Connected { .. })
    }

    /// Returns the local address if connected
    pub fn local_addr(&self) -> Option<&str> {
        match self {
            TunnelState::Connected { local_addr } => Some(local_addr),
            _ => None,
        }
    }
}

/// Managed tunnel with state tracking
#[allow(dead_code)]
struct ManagedTunnel {
    tunnel: SshTunnel,
    #[allow(dead_code)]
    config: SshTunnelConfig,
    state: TunnelState,
}

/// Global service for managing SSH tunnels.
///
/// Integrates with GPUI for:
/// - State updates for UI feedback
/// - Lifecycle management
/// - Keychain storage for SSH passwords
#[allow(dead_code)]
pub struct SshService {
    tunnels: HashMap<TunnelId, ManagedTunnel>,
    /// Channel for broadcasting state changes
    state_tx: Sender<(TunnelId, TunnelState)>,
    state_rx: Receiver<(TunnelId, TunnelState)>,
}

impl Global for SshService {}

#[allow(dead_code)]
impl SshService {
    /// Create a new SSH service
    pub fn new() -> Self {
        // Use larger buffer to prevent blocking on state updates
        let (state_tx, state_rx) = async_channel::bounded(256);
        Self {
            tunnels: HashMap::new(),
            state_tx,
            state_rx,
        }
    }

    /// Initialize as a global service
    pub fn init(cx: &mut gpui::App) {
        cx.set_global(Self::new());
    }

    /// Subscribe to tunnel state changes
    pub fn subscribe(&self) -> Receiver<(TunnelId, TunnelState)> {
        self.state_rx.clone()
    }

    /// Get a clone of the state sender for use in async tasks
    pub fn state_sender(&self) -> Sender<(TunnelId, TunnelState)> {
        self.state_tx.clone()
    }

    /// Get stored SSH password from keychain
    pub fn get_stored_password(ssh_host: &str, ssh_port: u16, ssh_user: &str) -> Option<String> {
        let key = Self::keychain_key(ssh_host, ssh_port, ssh_user);
        Entry::new(SSH_KEYRING_SERVICE, &key)
            .ok()
            .and_then(|e| e.get_password().ok())
    }

    /// Store SSH password in keychain
    pub fn store_password(
        ssh_host: &str,
        ssh_port: u16,
        ssh_user: &str,
        password: &str,
    ) -> Result<()> {
        let key = Self::keychain_key(ssh_host, ssh_port, ssh_user);
        let entry = Entry::new(SSH_KEYRING_SERVICE, &key)
            .context("Failed to create keyring entry for SSH password")?;
        entry
            .set_password(password)
            .context("Failed to store SSH password in keychain")?;
        Ok(())
    }

    /// Delete SSH password from keychain
    pub fn delete_password(ssh_host: &str, ssh_port: u16, ssh_user: &str) -> Result<()> {
        let key = Self::keychain_key(ssh_host, ssh_port, ssh_user);
        if let Ok(entry) = Entry::new(SSH_KEYRING_SERVICE, &key) {
            let _ = entry.delete_credential();
        }
        Ok(())
    }

    /// Generate keychain key for SSH credentials
    fn keychain_key(ssh_host: &str, ssh_port: u16, ssh_user: &str) -> String {
        format!("{}@{}:{}", ssh_user, ssh_host, ssh_port)
    }

    /// Generate keychain key for SSH key passphrases
    /// Uses a different format to distinguish from SSH passwords
    fn keychain_key_passphrase(
        ssh_host: &str,
        ssh_port: u16,
        ssh_user: &str,
        key_path: &str,
    ) -> String {
        // Use the key filename to distinguish passphrases for different keys
        let key_name = std::path::Path::new(key_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("key");
        format!("key:{}@{}:{}:{}", ssh_user, ssh_host, ssh_port, key_name)
    }

    /// Get stored SSH key passphrase from keychain
    pub fn get_stored_key_passphrase(
        ssh_host: &str,
        ssh_port: u16,
        ssh_user: &str,
        key_path: &str,
    ) -> Option<String> {
        let key = Self::keychain_key_passphrase(ssh_host, ssh_port, ssh_user, key_path);
        Entry::new(SSH_KEYRING_SERVICE, &key)
            .ok()
            .and_then(|e| e.get_password().ok())
    }

    /// Store SSH key passphrase in keychain
    pub fn store_key_passphrase(
        ssh_host: &str,
        ssh_port: u16,
        ssh_user: &str,
        key_path: &str,
        passphrase: &str,
    ) -> Result<()> {
        let key = Self::keychain_key_passphrase(ssh_host, ssh_port, ssh_user, key_path);
        let entry = Entry::new(SSH_KEYRING_SERVICE, &key)
            .context("Failed to create keyring entry for SSH key passphrase")?;
        entry
            .set_password(passphrase)
            .context("Failed to store SSH key passphrase in keychain")?;
        Ok(())
    }

    /// Delete SSH key passphrase from keychain
    pub fn delete_key_passphrase(
        ssh_host: &str,
        ssh_port: u16,
        ssh_user: &str,
        key_path: &str,
    ) -> Result<()> {
        let key = Self::keychain_key_passphrase(ssh_host, ssh_port, ssh_user, key_path);
        if let Ok(entry) = Entry::new(SSH_KEYRING_SERVICE, &key) {
            let _ = entry.delete_credential();
        }
        Ok(())
    }

    /// Create and start a tunnel with the given configuration.
    ///
    /// This is an async method intended to be called from within a `cx.spawn()` block.
    /// If password auth is needed and no password is in the config,
    /// attempts to load from keychain first.
    ///
    /// Returns the tunnel ID and the tunnel itself on success.
    pub async fn create_tunnel(
        mut config: SshTunnelConfig,
        state_tx: Sender<(TunnelId, TunnelState)>,
    ) -> Result<(TunnelId, SshTunnel, SshTunnelConfig)> {
        let tunnel_id = Uuid::new_v4();

        // Try to load credentials from keychain if not provided
        match &config.auth_method {
            SshAuthMethod::Password(password) if password.is_empty() => {
                if let Some(stored) = Self::get_stored_password(
                    &config.ssh_host.to_string(),
                    config.ssh_port,
                    &config.ssh_user,
                ) {
                    config.auth_method = SshAuthMethod::Password(stored);
                }
            }
            SshAuthMethod::PublicKey {
                private_key_path,
                passphrase,
            } if passphrase.is_none() => {
                if let Some(stored) = Self::get_stored_key_passphrase(
                    &config.ssh_host.to_string(),
                    config.ssh_port,
                    &config.ssh_user,
                    private_key_path,
                ) {
                    config.auth_method = SshAuthMethod::PublicKey {
                        private_key_path: private_key_path.clone(),
                        passphrase: Some(stored),
                    };
                }
            }
            _ => {}
        }

        // Notify connecting state
        let _ = state_tx.send((tunnel_id, TunnelState::Connecting)).await;

        match SshTunnel::start(config.clone()).await {
            Ok(tunnel) => {
                let local_addr = tunnel.local_addr();

                // Store credentials on successful connection if provided
                match &config.auth_method {
                    SshAuthMethod::Password(password) if !password.is_empty() => {
                        let _ = Self::store_password(
                            &config.ssh_host.to_string(),
                            config.ssh_port,
                            &config.ssh_user,
                            password,
                        );
                    }
                    SshAuthMethod::PublicKey {
                        private_key_path,
                        passphrase: Some(passphrase),
                    } if !passphrase.is_empty() => {
                        let _ = Self::store_key_passphrase(
                            &config.ssh_host.to_string(),
                            config.ssh_port,
                            &config.ssh_user,
                            private_key_path,
                            passphrase,
                        );
                    }
                    _ => {}
                }

                // Notify connected state
                let _ = state_tx
                    .send((
                        tunnel_id,
                        TunnelState::Connected {
                            local_addr: local_addr.clone(),
                        },
                    ))
                    .await;

                tracing::info!(
                    "SSH tunnel {} established: {} -> {}:{}",
                    tunnel_id,
                    local_addr,
                    config.remote_host,
                    config.remote_port
                );

                Ok((tunnel_id, tunnel, config))
            }
            Err(e) => {
                let error_msg = e.to_string();
                let _ = state_tx
                    .send((
                        tunnel_id,
                        TunnelState::Failed {
                            error: error_msg.clone(),
                        },
                    ))
                    .await;
                Err(e)
            }
        }
    }

    /// Create and start a tunnel with automatic retry on transient failures.
    ///
    /// This wraps `create_tunnel` with exponential backoff retry logic.
    /// It will retry on transient errors (network issues, port conflicts) but
    /// fail immediately on permanent errors (authentication failures).
    ///
    /// The `state_tx` channel will receive:
    /// - `TunnelState::Connecting` on first attempt
    /// - `TunnelState::Reconnecting` on retry attempts
    /// - `TunnelState::Connected` on success
    /// - `TunnelState::Failed` when all retries exhausted or permanent error
    pub async fn create_tunnel_with_retry(
        config: SshTunnelConfig,
        state_tx: Sender<(TunnelId, TunnelState)>,
        reconnect_config: ReconnectConfig,
    ) -> Result<(TunnelId, SshTunnel, SshTunnelConfig)> {
        let mut backoff = ExponentialBackoff::new(reconnect_config);
        let max_attempts = backoff.max_attempts();
        let tunnel_id = Uuid::new_v4();

        // Prepare config with keychain credentials if needed
        let mut config = config;
        match &config.auth_method {
            SshAuthMethod::Password(password) if password.is_empty() => {
                if let Some(stored) = Self::get_stored_password(
                    &config.ssh_host.to_string(),
                    config.ssh_port,
                    &config.ssh_user,
                ) {
                    config.auth_method = SshAuthMethod::Password(stored);
                }
            }
            SshAuthMethod::PublicKey {
                private_key_path,
                passphrase,
            } if passphrase.is_none() => {
                if let Some(stored) = Self::get_stored_key_passphrase(
                    &config.ssh_host.to_string(),
                    config.ssh_port,
                    &config.ssh_user,
                    private_key_path,
                ) {
                    config.auth_method = SshAuthMethod::PublicKey {
                        private_key_path: private_key_path.clone(),
                        passphrase: Some(stored),
                    };
                }
            }
            _ => {}
        }

        #[allow(unused_assignments)]
        let mut last_error: Option<anyhow::Error> = None;

        loop {
            let attempt = backoff.attempt();

            // Send appropriate state
            if attempt == 0 {
                let _ = state_tx.send((tunnel_id, TunnelState::Connecting)).await;
            } else {
                let _ = state_tx
                    .send((
                        tunnel_id,
                        TunnelState::Reconnecting {
                            attempt,
                            max_attempts,
                        },
                    ))
                    .await;
            }

            // Try to create the tunnel
            match SshTunnel::start(config.clone()).await {
                Ok(tunnel) => {
                    let local_addr = tunnel.local_addr();

                    // Store credentials on successful connection
                    match &config.auth_method {
                        SshAuthMethod::Password(password) if !password.is_empty() => {
                            let _ = Self::store_password(
                                &config.ssh_host.to_string(),
                                config.ssh_port,
                                &config.ssh_user,
                                password,
                            );
                        }
                        SshAuthMethod::PublicKey {
                            private_key_path,
                            passphrase: Some(passphrase),
                        } if !passphrase.is_empty() => {
                            let _ = Self::store_key_passphrase(
                                &config.ssh_host.to_string(),
                                config.ssh_port,
                                &config.ssh_user,
                                private_key_path,
                                passphrase,
                            );
                        }
                        _ => {}
                    }

                    let _ = state_tx
                        .send((
                            tunnel_id,
                            TunnelState::Connected {
                                local_addr: local_addr.clone(),
                            },
                        ))
                        .await;

                    tracing::info!(
                        "SSH tunnel {} established: {} -> {}:{}{}",
                        tunnel_id,
                        local_addr,
                        config.remote_host,
                        config.remote_port,
                        if attempt > 0 {
                            format!(" (after {} retries)", attempt)
                        } else {
                            String::new()
                        }
                    );

                    return Ok((tunnel_id, tunnel, config));
                }
                Err(e) => {
                    // Check if error is retriable
                    if !is_retriable_error(&e) {
                        tracing::warn!(
                            "SSH tunnel {} failed with non-retriable error: {}",
                            tunnel_id,
                            e
                        );
                        let _ = state_tx
                            .send((
                                tunnel_id,
                                TunnelState::Failed {
                                    error: e.to_string(),
                                },
                            ))
                            .await;
                        return Err(e);
                    }

                    last_error = Some(e);

                    // Get next delay, or give up if max attempts reached
                    match backoff.next_delay() {
                        Some(delay) => {
                            tracing::info!(
                                "SSH tunnel {} attempt {} failed, retrying in {:?}...",
                                tunnel_id,
                                backoff.attempt(),
                                delay
                            );
                            smol::Timer::after(delay).await;
                        }
                        None => {
                            // Max attempts reached
                            let error = last_error.unwrap_or_else(|| {
                                anyhow::anyhow!(
                                    "Max reconnection attempts ({}) exceeded",
                                    max_attempts
                                )
                            });
                            tracing::error!(
                                "SSH tunnel {} failed after {} attempts: {}",
                                tunnel_id,
                                max_attempts,
                                error
                            );
                            let _ = state_tx
                                .send((
                                    tunnel_id,
                                    TunnelState::Failed {
                                        error: error.to_string(),
                                    },
                                ))
                                .await;
                            return Err(error);
                        }
                    }
                }
            }
        }
    }

    /// Register a successfully created tunnel.
    /// Called after create_tunnel succeeds.
    pub fn register_tunnel(&mut self, id: TunnelId, tunnel: SshTunnel, config: SshTunnelConfig) {
        let local_addr = tunnel.local_addr();
        self.tunnels.insert(
            id,
            ManagedTunnel {
                tunnel,
                config,
                state: TunnelState::Connected { local_addr },
            },
        );
    }

    /// Remove a tunnel from the service and return it for async shutdown.
    /// This is a synchronous operation that can be called from update_global.
    /// The caller is responsible for calling shutdown() on the returned tunnel.
    pub fn remove_tunnel(&mut self, id: TunnelId) -> Option<SshTunnel> {
        self.tunnels.remove(&id).map(|managed| {
            tracing::debug!("Removed tunnel {} from service", id);
            managed.tunnel
        })
    }

    /// Get local address for a tunnel (for database connection)
    pub fn local_addr(&self, id: TunnelId) -> Option<String> {
        self.tunnels.get(&id).map(|t| t.tunnel.local_addr())
    }

    /// Get tunnel state for UI display
    pub fn tunnel_state(&self, id: TunnelId) -> Option<TunnelState> {
        self.tunnels.get(&id).map(|t| t.state.clone())
    }

    /// Check if a tunnel is healthy
    pub fn is_tunnel_healthy(&mut self, id: TunnelId) -> bool {
        self.tunnels
            .get_mut(&id)
            .map(|t| t.tunnel.is_alive() && t.state.is_connected())
            .unwrap_or(false)
    }

    /// Get all active tunnel IDs
    pub fn active_tunnels(&self) -> Vec<TunnelId> {
        self.tunnels
            .iter()
            .filter(|(_, t)| t.state.is_connected())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Close a specific tunnel
    pub async fn close_tunnel(&mut self, id: TunnelId) {
        if let Some(managed) = self.tunnels.remove(&id) {
            let _ = self.state_tx.send((id, TunnelState::Closed)).await;
            managed.tunnel.shutdown().await;
            tracing::info!("SSH tunnel {} closed", id);
        }
    }

    /// Close all tunnels (called on app shutdown)
    pub async fn shutdown(&mut self) {
        let ids: Vec<_> = self.tunnels.keys().cloned().collect();
        for id in ids {
            self.close_tunnel(id).await;
        }
        tracing::info!("All SSH tunnels shut down");
    }
}

impl Default for SshService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_tunnel_state_is_connected() {
        assert!(!TunnelState::Connecting.is_connected());
        assert!(
            TunnelState::Connected {
                local_addr: "127.0.0.1:5432".to_string()
            }
            .is_connected()
        );
        assert!(
            !TunnelState::Reconnecting {
                attempt: 1,
                max_attempts: 5
            }
            .is_connected()
        );
        assert!(
            !TunnelState::Failed {
                error: "test".to_string()
            }
            .is_connected()
        );
        assert!(!TunnelState::Closed.is_connected());
    }

    #[test]
    fn test_tunnel_state_local_addr() {
        assert_eq!(TunnelState::Connecting.local_addr(), None);
        assert_eq!(
            TunnelState::Connected {
                local_addr: "127.0.0.1:5432".to_string()
            }
            .local_addr(),
            Some("127.0.0.1:5432")
        );
        assert_eq!(
            TunnelState::Reconnecting {
                attempt: 1,
                max_attempts: 5
            }
            .local_addr(),
            None
        );
        assert_eq!(
            TunnelState::Failed {
                error: "test".to_string()
            }
            .local_addr(),
            None
        );
        assert_eq!(TunnelState::Closed.local_addr(), None);
    }

    #[test]
    fn test_tunnel_state_equality() {
        // Verify PartialEq works correctly
        assert_eq!(TunnelState::Connecting, TunnelState::Connecting);
        assert_eq!(TunnelState::Closed, TunnelState::Closed);
        assert_eq!(
            TunnelState::Connected {
                local_addr: "127.0.0.1:1234".to_string()
            },
            TunnelState::Connected {
                local_addr: "127.0.0.1:1234".to_string()
            }
        );
        assert_ne!(
            TunnelState::Connected {
                local_addr: "127.0.0.1:1234".to_string()
            },
            TunnelState::Connected {
                local_addr: "127.0.0.1:5678".to_string()
            }
        );
        assert_eq!(
            TunnelState::Reconnecting {
                attempt: 2,
                max_attempts: 5
            },
            TunnelState::Reconnecting {
                attempt: 2,
                max_attempts: 5
            }
        );
        assert_ne!(
            TunnelState::Reconnecting {
                attempt: 1,
                max_attempts: 5
            },
            TunnelState::Reconnecting {
                attempt: 2,
                max_attempts: 5
            }
        );
    }

    #[test]
    fn test_ssh_service_new() {
        let service = SshService::new();
        assert!(service.tunnels.is_empty());
    }

    #[test]
    fn test_keychain_key_format() {
        let key = SshService::keychain_key("example.com", 22, "testuser");
        assert_eq!(key, "testuser@example.com:22");

        let key_custom_port = SshService::keychain_key("db.example.com", 2222, "admin");
        assert_eq!(key_custom_port, "admin@db.example.com:2222");
    }

    #[test]
    fn test_keychain_key_passphrase_format() {
        let key = SshService::keychain_key_passphrase(
            "example.com",
            22,
            "testuser",
            "/home/user/.ssh/id_rsa",
        );
        assert_eq!(key, "key:testuser@example.com:22:id_rsa");

        let key_ed25519 = SshService::keychain_key_passphrase(
            "example.com",
            22,
            "testuser",
            "/home/user/.ssh/id_ed25519",
        );
        assert_eq!(key_ed25519, "key:testuser@example.com:22:id_ed25519");

        // Different keys for same host should have different keychain keys
        assert_ne!(key, key_ed25519);
    }

    #[test]
    fn test_reconnect_config_default() {
        let config = ReconnectConfig::default();
        assert_eq!(config.initial_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.multiplier, 2.0);
        assert_eq!(config.max_attempts, Some(5));
    }
}
