//! Actions that orchestrate state changes across multiple global states.
//!
//! These functions handle cross-cutting concerns like connecting/disconnecting
//! from databases, which need to update multiple states simultaneously.

use std::time::Duration;

use gpui::*;

use crate::services::ssh::{ReconnectConfig, SshAuthMethod, SshService, SshTunnelConfig, TunnelId};
use crate::services::{
    AppStore, ConnectionInfo, ConnectionsRepository, DatabaseManager, SshAuthType,
};

use super::connection::{ConnectionState, ConnectionStatus};
use super::database::DatabaseState;
use super::editor::EditorState;

// =============================================================================
// Connection Lifecycle
// =============================================================================

/// Initiates a connection to the database.
/// Updates ConnectionState, EditorState, and DatabaseState on success.
/// If SSH tunnel is configured, establishes tunnel first.
pub fn connect(connection_info: &ConnectionInfo, cx: &mut App) {
    cx.update_global::<ConnectionState, _>(|state, _cx| {
        state.connection_state = ConnectionStatus::Connecting;
    });

    let cic = connection_info.clone();
    let db_manager = cx.global::<ConnectionState>().db_manager.clone();

    // Check if we need SSH tunnel
    if cic.uses_ssh_tunnel() {
        let state_tx = cx.global::<SshService>().state_sender();

        cx.spawn(async move |cx| {
            // Update state to show we're establishing tunnel
            let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
                state.connection_state = ConnectionStatus::ConnectingTunnel;
            });

            // Load DB password from keychain
            let mut cic = cic;
            if let Ok(password) = ConnectionsRepository::get_connection_password(&cic.id) {
                cic.password = password;
            } else {
                let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
                    state.connection_state = ConnectionStatus::Disconnected;
                });
                tracing::error!("Failed to get database password from keychain");
                return;
            }

            // Build SSH tunnel config from connection info
            let ssh_tunnel = cic.ssh_tunnel.as_ref().unwrap();
            let tunnel_config = build_tunnel_config(&cic, ssh_tunnel);

            // Create the SSH tunnel with automatic retry on transient failures
            match SshService::create_tunnel_with_retry(
                tunnel_config,
                state_tx,
                ReconnectConfig::default(),
            )
            .await
            {
                Ok((tunnel_id, tunnel, _config)) => {
                    let local_addr = tunnel.local_addr();
                    tracing::info!("SSH tunnel established at {}", local_addr);

                    // Register the tunnel with the service
                    let _ = cx.update_global::<SshService, _>(|service, _cx| {
                        service.register_tunnel(tunnel_id, tunnel, _config);
                    });

                    // Store tunnel ID
                    let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
                        state.active_tunnel_id = Some(tunnel_id);
                        state.connection_state = ConnectionStatus::Connecting;
                    });

                    // Parse local address for database connection
                    let parts: Vec<&str> = local_addr.split(':').collect();
                    let tunnel_host = parts[0].to_string();
                    let tunnel_port: u16 = parts[1].parse().unwrap_or(5432);

                    // Connect through the tunnel
                    connect_async_via_tunnel(
                        cic,
                        tunnel_host,
                        tunnel_port,
                        tunnel_id,
                        db_manager,
                        cx.clone(),
                    )
                    .await;
                }
                Err(e) => {
                    tracing::error!("Failed to create SSH tunnel: {}", e);
                    let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
                        state.connection_state = ConnectionStatus::Disconnected;
                    });
                }
            }
        })
        .detach();
    } else {
        // Direct connection without SSH tunnel
        cx.spawn(async move |cx| connect_async(cic, db_manager, cx).await)
            .detach();
    }
}

/// Build SshTunnelConfig from ConnectionInfo and SshTunnelInfo
fn build_tunnel_config(
    connection: &ConnectionInfo,
    ssh_tunnel: &crate::services::SshTunnelInfo,
) -> SshTunnelConfig {
    let auth_method = match ssh_tunnel.auth_type {
        SshAuthType::Agent => SshAuthMethod::Agent,
        SshAuthType::Password => {
            // Try to load SSH password from keychain
            let password = SshService::get_stored_password(
                &ssh_tunnel.ssh_host,
                ssh_tunnel.ssh_port,
                &ssh_tunnel.ssh_user,
            )
            .unwrap_or_default();
            SshAuthMethod::Password(password)
        }
        SshAuthType::PublicKey => {
            let private_key_path = ssh_tunnel.private_key_path.clone().unwrap_or_default();
            // Try to load key passphrase from keychain
            let passphrase = SshService::get_stored_key_passphrase(
                &ssh_tunnel.ssh_host,
                ssh_tunnel.ssh_port,
                &ssh_tunnel.ssh_user,
                &private_key_path,
            );
            SshAuthMethod::PublicKey {
                private_key_path,
                passphrase,
            }
        }
    };

    SshTunnelConfig {
        ssh_host: ssh_tunnel.ssh_host.clone(),
        ssh_port: ssh_tunnel.ssh_port,
        ssh_user: ssh_tunnel.ssh_user.clone(),
        auth_method,
        remote_host: connection.hostname.clone(),
        remote_port: connection.port as u16,
        local_bind_host: "127.0.0.1".to_string(),
        local_bind_port: 0, // Auto-assign
        extra_args: vec![],
    }
}

/// Disconnects from the current database.
/// Also closes any active SSH tunnel.
pub fn disconnect(cx: &mut App) {
    let db_manager = cx.global::<ConnectionState>().db_manager.clone();
    let tunnel_id = cx.global::<ConnectionState>().active_tunnel_id;

    // Remove tunnel from service synchronously, then shut it down async
    let tunnel = tunnel_id
        .and_then(|id| cx.update_global::<SshService, _>(|service, _cx| service.remove_tunnel(id)));

    cx.spawn(async move |cx| {
        // Close SSH tunnel if we have one
        if let Some(tunnel) = tunnel {
            tracing::info!("Shutting down SSH tunnel");
            tunnel.shutdown().await;
            tracing::info!("SSH tunnel closed");
        }

        disconnect_async(db_manager, cx).await;

        let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
            state.active_tunnel_id = None;
        });
    })
    .detach();
}

/// Changes to a different database on the same server.
/// Disconnects from current database and reconnects to the new one.
/// Preserves existing SSH tunnel if one is active.
pub fn change_database(database_name: String, cx: &mut App) {
    let current_connection = cx.global::<ConnectionState>().active_connection.clone();
    let tunnel_id = cx.global::<ConnectionState>().active_tunnel_id.clone();

    // Get tunnel local address if we have an active tunnel
    let tunnel_addr = tunnel_id.and_then(|id| cx.global::<SshService>().local_addr(id).clone());

    if let Some(mut new_connection) = current_connection {
        new_connection.database = database_name;

        let db_manager = cx.global::<ConnectionState>().db_manager.clone();
        cx.spawn(async move |cx| {
            disconnect_async(db_manager.clone(), cx).await;
            // Wait a brief moment for cleanup
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;

            // If we have an active SSH tunnel, connect through it
            if let (Some(tunnel_id), Some(addr)) = (tunnel_id, tunnel_addr) {
                let parts: Vec<&str> = addr.split(':').collect();
                if parts.len() == 2 {
                    let tunnel_host = parts[0].to_string();
                    let tunnel_port: u16 = parts[1].parse().unwrap_or(5432);

                    connect_async_via_tunnel(
                        new_connection,
                        tunnel_host,
                        tunnel_port,
                        tunnel_id,
                        db_manager,
                        cx.clone(),
                    )
                    .await;
                    return;
                }
            }

            // Direct connection (no SSH tunnel)
            connect_async(new_connection, db_manager, cx).await;
        })
        .detach();
    }
}

// =============================================================================
// Connection CRUD Operations
// =============================================================================

/// Adds a new connection to the saved connections store.
pub fn add_connection(connection: ConnectionInfo, cx: &mut App) {
    cx.spawn(async move |cx| {
        if let Ok(store) = AppStore::singleton().await {
            if let Ok(_) = store.connections().create(&connection).await {
                if let Ok(connections) = store.connections().load_all().await {
                    let _ = cx.update_global::<ConnectionState, _>(|app_state, _cx| {
                        app_state.saved_connections = connections;
                        app_state.active_connection = None;
                    });
                }
            }
        }
    })
    .detach();
}

/// Updates an existing connection in the saved connections store.
pub fn update_connection(connection: ConnectionInfo, cx: &mut App) {
    cx.spawn(async move |cx| {
        if let Ok(store) = AppStore::singleton().await {
            if let Ok(_) = store.connections().update(&connection).await {
                if let Ok(connections) = store.connections().load_all().await {
                    let _ = cx.update_global::<ConnectionState, _>(|app_state, _cx| {
                        app_state.saved_connections = connections;
                        app_state.active_connection = Some(connection);
                    });
                }
            }
        }
    })
    .detach();
}

/// Deletes a connection from the saved connections store.
pub fn delete_connection(connection: ConnectionInfo, cx: &mut App) {
    let conn = connection.clone();
    cx.spawn(async move |cx| {
        if let Ok(store) = AppStore::singleton().await {
            if let Ok(_) = store.connections().delete(&conn.id).await {
                if let Ok(connections) = store.connections().load_all().await {
                    let _ = cx.update_global::<ConnectionState, _>(|app_state, _cx| {
                        app_state.saved_connections = connections;
                    });
                }
            }
        }
    })
    .detach();
}

// =============================================================================
// Private Async Helpers
// =============================================================================

async fn connect_async(mut cic: ConnectionInfo, db_manager: DatabaseManager, cx: &mut AsyncApp) {
    // Load password from keychain on-demand
    if let Ok(password) = ConnectionsRepository::get_connection_password(&cic.id) {
        cic.password = password;
    } else {
        let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
            state.connection_state = ConnectionStatus::Disconnected;
        });
        return;
    }

    // Use secure connection options instead of string
    let connect_options = cic.to_pg_connect_options();

    if let Ok(_) = db_manager.connect_with_options(connect_options).await {
        finish_connection(cic, db_manager, cx).await;
    } else {
        tracing::warn!("No Connect :(");
        let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
            state.active_connection = None;
            state.connection_state = ConnectionStatus::Disconnected;
        });
    }
}

async fn connect_async_via_tunnel(
    cic: ConnectionInfo,
    tunnel_host: String,
    tunnel_port: u16,
    tunnel_id: TunnelId,
    db_manager: DatabaseManager,
    mut cx: AsyncApp,
) {
    // Create connection options pointing to the tunnel
    let connect_options = cic.to_pg_connect_options_with_tunnel(&tunnel_host, tunnel_port);

    if let Ok(_) = db_manager.connect_with_options(connect_options).await {
        finish_connection(cic, db_manager, &mut cx).await;
    } else {
        tracing::warn!("Failed to connect through SSH tunnel");

        // Close the tunnel since we couldn't use it
        let tunnel = cx
            .update_global::<SshService, _>(|service, _cx| service.remove_tunnel(tunnel_id))
            .ok()
            .flatten();

        if let Some(tunnel) = tunnel {
            tunnel.shutdown().await;
            tracing::info!("Closed unused SSH tunnel: {}", tunnel_id);
        }

        let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
            state.active_connection = None;
            state.active_tunnel_id = None;
            state.connection_state = ConnectionStatus::Disconnected;
        });
    }
}

async fn finish_connection(cic: ConnectionInfo, db_manager: DatabaseManager, cx: &mut AsyncApp) {
    if let Ok(tables) = db_manager.get_tables().await {
        let _ = cx.update_global::<EditorState, _>(|state, _cx| {
            state.tables = tables;
        });
    }

    if let Ok(schema) = db_manager.get_schema(None).await {
        let _ = cx.update_global::<EditorState, _>(|state, _cx| {
            state.schema = Some(schema);
        });
    }

    if let Ok(databases) = db_manager.get_databases().await {
        let _ = cx.update_global::<DatabaseState, _>(|state, _cx| {
            state.databases = databases;
        });
    }

    let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
        state.active_connection = Some(cic);
        state.connection_state = ConnectionStatus::Connected;
    });

    // Connection monitoring loop
    loop {
        let mut connected = db_manager.is_connected().await;
        if !connected {
            let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
                state.active_connection = None;
                state.connection_state = ConnectionStatus::Disconnected;
            });
            break;
        }

        let _ = cx.try_read_global::<ConnectionState, _>(|state, _cx| {
            if state.active_connection.is_none() {
                connected = false;
            }
        });

        if !connected {
            break;
        }

        cx.background_executor()
            .timer(Duration::from_millis(1000))
            .await;
    }
}

async fn disconnect_async(db_manager: DatabaseManager, cx: &mut AsyncApp) {
    let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
        state.active_connection = None;
        state.connection_state = ConnectionStatus::Disconnecting;
    });

    if let Ok(_) = db_manager.disconnect().await {
        let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
            state.active_connection = None;
            state.connection_state = ConnectionStatus::Disconnected;
        });
    }
}
