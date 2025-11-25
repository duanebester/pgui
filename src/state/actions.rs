//! Actions that orchestrate state changes across multiple global states.
//!
//! These functions handle cross-cutting concerns like connecting/disconnecting
//! from databases, which need to update multiple states simultaneously.

use std::time::Duration;

use gpui::*;

use crate::services::{ConnectionInfo, ConnectionsStore, DatabaseManager};

use super::connection::{ConnectionState, ConnectionStatus};
use super::database::DatabaseState;
use super::editor::EditorState;
use super::llm::LLMState;

// =============================================================================
// Connection Lifecycle
// =============================================================================

/// Initiates a connection to the database.
/// Updates ConnectionState, EditorState, DatabaseState, and LLMState on success.
pub fn connect(connection_info: &ConnectionInfo, cx: &mut App) {
    cx.update_global::<ConnectionState, _>(|state, _cx| {
        state.connection_state = ConnectionStatus::Connecting;
    });

    let cic = connection_info.clone();
    let db_manager = cx.global::<ConnectionState>().db_manager.clone();

    cx.spawn(async move |cx| connect_async(cic, db_manager, cx).await)
        .detach();
}

/// Disconnects from the current database.
/// Updates ConnectionState and LLMState.
pub fn disconnect(cx: &mut App) {
    let db_manager = cx.global::<ConnectionState>().db_manager.clone();
    cx.spawn(async move |cx| disconnect_async(db_manager, cx).await)
        .detach();
}

/// Changes to a different database on the same server.
/// Disconnects from current database and reconnects to the new one.
pub fn change_database(database_name: String, cx: &mut App) {
    let current_connection = cx.global::<ConnectionState>().active_connection.clone();

    if let Some(mut new_connection) = current_connection {
        new_connection.database = database_name;

        let db_manager = cx.global::<ConnectionState>().db_manager.clone();
        cx.spawn(async move |cx| {
            disconnect_async(db_manager.clone(), cx).await;
            // Wait a brief moment for cleanup
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            // Connect to the new database
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

/// Updates an existing connection in the saved connections store.
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

/// Deletes a connection from the saved connections store.
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

// =============================================================================
// Private Async Helpers
// =============================================================================

async fn connect_async(mut cic: ConnectionInfo, db_manager: DatabaseManager, cx: &mut AsyncApp) {
    // Load password from keychain on-demand
    if let Ok(password) = ConnectionsStore::get_connection_password(&cic.id) {
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
        if let Ok(tables) = db_manager.get_tables().await {
            let _ = cx.update_global::<EditorState, _>(|state, _cx| {
                state.tables = tables;
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

        if let Ok(schema) = db_manager.get_schema(None).await {
            let llm_schema = Some(db_manager.format_schema_for_llm(&schema).into());
            let _ = cx.update_global::<LLMState, _>(|state, _cx| {
                state.llm_schema = llm_schema;
            });
        }

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
    } else {
        println!("No Connect :(");
        let _ = cx.update_global::<ConnectionState, _>(|state, _cx| {
            state.active_connection = None;
            state.connection_state = ConnectionStatus::Disconnected;
        });
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
        let _ = cx.update_global::<LLMState, _>(|state, _cx| {
            state.llm_schema = None;
        });
    }
}
