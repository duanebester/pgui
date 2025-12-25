//! SSH tunnel management service with GPUI integration.
//!
//! This module provides:
//! - `SshService` - Global service for managing SSH tunnels
//! - `TunnelState` - Lifecycle states for UI feedback
//! - Keychain integration for SSH passwords

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
    Closed,
}

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
pub struct SshService {
    tunnels: HashMap<TunnelId, ManagedTunnel>,
    /// Channel for broadcasting state changes
    state_tx: Sender<(TunnelId, TunnelState)>,
    state_rx: Receiver<(TunnelId, TunnelState)>,
}

impl Global for SshService {}

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

        // Try to load password from keychain if using password auth without one
        if let SshAuthMethod::Password(ref password) = config.auth_method {
            if password.is_empty() {
                if let Some(stored) =
                    Self::get_stored_password(&config.ssh_host, config.ssh_port, &config.ssh_user)
                {
                    config.auth_method = SshAuthMethod::Password(stored);
                }
            }
        }

        // Notify connecting state
        let _ = state_tx.send((tunnel_id, TunnelState::Connecting)).await;

        match SshTunnel::start(config.clone()).await {
            Ok(tunnel) => {
                let local_addr = tunnel.local_addr();

                // Store password on successful connection if it was provided
                if let SshAuthMethod::Password(ref password) = config.auth_method {
                    if !password.is_empty() {
                        let _ = Self::store_password(
                            &config.ssh_host,
                            config.ssh_port,
                            &config.ssh_user,
                            password,
                        );
                    }
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
