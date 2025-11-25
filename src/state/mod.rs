//! Application state management.
//!
//! This module contains global state structs and actions that orchestrate
//! state changes across multiple domains.
//!
//! ## Structure
//!
//! - `connection` - Connection status and saved connections
//! - `database` - Available databases on the connected server
//! - `editor` - Editor-related state (tables for autocomplete, etc.)
//! - `llm` - LLM/Agent-related state (schema for context)
//! - `actions` - Cross-cutting operations (connect, disconnect, etc.)

mod actions;
mod connection;
mod database;
mod editor;
mod llm;

// Re-export state structs
pub use connection::{ConnectionState, ConnectionStatus};
pub use database::DatabaseState;
pub use editor::EditorState;
pub use llm::LLMState;

// Re-export actions for orchestration
pub use actions::{
    add_connection, change_database, connect, delete_connection, disconnect, update_connection,
};

use gpui::App;

/// Initialize all global state.
pub fn init(cx: &mut App) {
    ConnectionState::init(cx);
    DatabaseState::init(cx);
    EditorState::init(cx);
    LLMState::init(cx);
}
