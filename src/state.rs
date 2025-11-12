use std::time::Duration;

use gpui::*;

use crate::services::{ConnectionInfo, ConnectionsStore, DatabaseManager, TableInfo};

#[derive(Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Disconnecting,
    Connecting,
    Connected,
}

pub struct LLMState {
    pub llm_schema: Option<SharedString>,
}

impl Global for LLMState {}

impl LLMState {
    pub fn init(cx: &mut App) {
        let this = LLMState { llm_schema: None };
        cx.set_global(this);
    }
}

pub struct EditorState {
    pub tables: Vec<TableInfo>,
}

impl Global for EditorState {}

impl EditorState {
    pub fn init(cx: &mut App) {
        let this = EditorState { tables: vec![] };
        cx.set_global(this);
    }
}

pub struct ConnectionState {
    pub saved_connections: Vec<ConnectionInfo>,
    pub active_connection: Option<ConnectionInfo>,
    pub db_manager: DatabaseManager,
    pub connection_state: ConnectionStatus,
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
        };
        cx.set_global(this);
        cx.spawn(async move |cx| {
            if let Ok(store) = ConnectionsStore::new().await {
                if let Ok(connections) = store.load_connections().await {
                    let _ = cx.update_global::<ConnectionState, _>(|app_state, _cx| {
                        app_state.saved_connections = connections;
                    });
                }
            }
        })
        .detach();
    }

    pub fn connect(connection_info: &ConnectionInfo, cx: &mut App) {
        let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
            state.connection_state = ConnectionStatus::Connecting;
        });
        let cic = connection_info.clone();
        let app_state = cx.global::<ConnectionState>();
        let db_manager = app_state.db_manager.clone();
        cx.spawn(async move |cx| {
            // Use secure connection options instead of string
            let connect_options = cic.to_pg_connect_options();

            if let Ok(_) = db_manager.connect_with_options(connect_options).await {
                let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
                    state.active_connection = Some(cic);
                    state.connection_state = ConnectionStatus::Connected;
                });

                if let Ok(schema) = db_manager.get_schema(None).await {
                    let llm_schema = Some(db_manager.format_schema_for_llm(&schema).into());
                    let _ = cx.update_global::<LLMState, _>(|state, _cx| {
                        state.llm_schema = llm_schema;
                    });
                }

                if let Ok(tables) = db_manager.get_tables().await {
                    let _ = cx.update_global::<EditorState, _>(|state, _cx| {
                        state.tables = tables;
                    });
                }

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

                    cx.background_executor()
                        .timer(Duration::from_millis(1000))
                        .await;
                }
            }
        })
        .detach();
    }

    pub fn disconnect(cx: &mut App) {
        let app_state = cx.global::<ConnectionState>();
        let db_manager = app_state.db_manager.clone();
        cx.spawn(async move |cx| {
            let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
                state.connection_state = ConnectionStatus::Disconnecting;
            });
            if let Ok(_) = db_manager.disconnect().await {
                let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
                    // TODO: default blank state?
                    state.active_connection = None;
                    state.connection_state = ConnectionStatus::Disconnected;
                });
                let _ = cx.update_global::<LLMState, _>(|state, _cx| {
                    // TODO: default blank state?
                    state.llm_schema = None;
                });
            }
        })
        .detach();
    }

    pub fn delete_connection(connection: ConnectionInfo, cx: &mut App) {
        let conn = connection.clone();
        cx.spawn(async move |cx| {
            if let Ok(store) = ConnectionsStore::new().await {
                if let Ok(_) = store.delete_connection(&conn.id).await {
                    if let Ok(connections) = store.load_connections().await {
                        let _ = cx.update_global::<ConnectionState, _>(|app_state, _cx| {
                            app_state.saved_connections = connections;
                        });
                    }
                }
            }
        })
        .detach();
    }

    pub fn add_connection(connection: ConnectionInfo, cx: &mut App) {
        cx.spawn(async move |cx| {
            if let Ok(store) = ConnectionsStore::new().await {
                if let Ok(_) = store.create_connection(&connection).await {
                    if let Ok(connections) = store.load_connections().await {
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

    pub fn update_connection(connection: ConnectionInfo, cx: &mut App) {
        cx.spawn(async move |cx| {
            if let Ok(store) = ConnectionsStore::new().await {
                if let Ok(_) = store.update_connection(&connection).await {
                    if let Ok(connections) = store.load_connections().await {
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
}
