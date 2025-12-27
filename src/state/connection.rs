use gpui::*;

use crate::services::ssh::TunnelId;
use crate::services::{AppStore, ConnectionInfo, DatabaseManager};

#[derive(Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Disconnecting,
    Connecting,
    ConnectingTunnel,
    Connected,
}

pub struct ConnectionState {
    pub saved_connections: Vec<ConnectionInfo>,
    pub active_connection: Option<ConnectionInfo>,
    pub db_manager: DatabaseManager,
    pub connection_state: ConnectionStatus,
    /// Active SSH tunnel ID for the current connection
    pub active_tunnel_id: Option<TunnelId>,
}

impl Global for ConnectionState {}

impl ConnectionState {
    pub fn init(cx: &mut App) {
        let db_manager = DatabaseManager::new();
        let this = ConnectionState {
            saved_connections: vec![],
            active_connection: None,
            db_manager,
            connection_state: ConnectionStatus::Disconnected,
            active_tunnel_id: None,
        };
        cx.set_global(this);

        // Load saved connections on startup
        cx.spawn(async move |cx| {
            if let Ok(store) = AppStore::singleton().await {
                if let Ok(connections) = store.connections().load_all().await {
                    let _ = cx.update_global::<ConnectionState, _>(|app_state, _cx| {
                        app_state.saved_connections = connections;
                    });
                }
            }
        })
        .detach();
    }
}
