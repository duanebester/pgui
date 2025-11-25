//! Connections module for managing PostgreSQL connection configurations.
//!
//! This module provides:
//! - `store` - SQLite-based connection storage with secure keyring password management
//! - `types` - Connection configuration types (ConnectionInfo, SslMode)

mod store;
mod types;

// Re-export store types
pub use store::ConnectionsStore;

// Re-export connection types
pub use types::{ConnectionInfo, SslMode};
