//! Connection type definitions.
//!
//! This module contains:
//! - `SslMode` - SSL mode options for PostgreSQL connections
//! - `ConnectionInfo` - PostgreSQL connection configuration
use chrono::{DateTime, Utc};
use gpui::SharedString;
use gpui_component::select::SelectItem;
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use uuid::Uuid;

/// SSL mode options for PostgreSQL connections
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl SelectItem for SslMode {
    type Value = &'static str;

    fn title(&self) -> SharedString {
        self.as_str().into()
    }

    fn value(&self) -> &Self::Value {
        match self {
            SslMode::Disable => &"disable",
            SslMode::Prefer => &"prefer",
            SslMode::Require => &"require",
            SslMode::VerifyCa => &"verify-ca",
            SslMode::VerifyFull => &"verify-full",
        }
    }
}

impl Default for SslMode {
    fn default() -> Self {
        SslMode::Prefer
    }
}

#[allow(dead_code)]
impl SslMode {
    /// Convert to sqlx PgSslMode
    pub fn to_pg_ssl_mode(&self) -> PgSslMode {
        match self {
            SslMode::Disable => PgSslMode::Disable,
            SslMode::Prefer => PgSslMode::Prefer,
            SslMode::Require => PgSslMode::Require,
            SslMode::VerifyCa => PgSslMode::VerifyCa,
            SslMode::VerifyFull => PgSslMode::VerifyFull,
        }
    }

    /// Get the display string for this SSL mode
    pub fn as_str(&self) -> &'static str {
        match self {
            SslMode::Disable => "Disable",
            SslMode::Prefer => "Prefer",
            SslMode::Require => "Require",
            SslMode::VerifyCa => "Verify CA",
            SslMode::VerifyFull => "Verify Full",
        }
    }

    /// Get a description of what this SSL mode does
    pub fn description(&self) -> &str {
        match self {
            SslMode::Disable => "No SSL connection",
            SslMode::Prefer => "Try SSL first, fall back to non-SSL",
            SslMode::Require => "Require SSL, don't verify certificates",
            SslMode::VerifyCa => "Require SSL and verify server certificate",
            SslMode::VerifyFull => "Require SSL, verify certificate and hostname",
        }
    }

    /// Get all available SSL modes
    pub fn all() -> Vec<SslMode> {
        vec![
            SslMode::Disable,
            SslMode::Prefer,
            SslMode::Require,
            SslMode::VerifyCa,
            SslMode::VerifyFull,
        ]
    }

    /// Create an SSL mode from a zero-based index
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => SslMode::Disable,
            1 => SslMode::Prefer,
            2 => SslMode::Require,
            3 => SslMode::VerifyCa,
            4 => SslMode::VerifyFull,
            _ => SslMode::Prefer,
        }
    }

    /// Convert this SSL mode to a zero-based index
    pub fn to_index(&self) -> usize {
        match self {
            SslMode::Disable => 0,
            SslMode::Prefer => 1,
            SslMode::Require => 2,
            SslMode::VerifyCa => 3,
            SslMode::VerifyFull => 4,
        }
    }

    /// Parse an SSL mode from a database string
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "disable" => SslMode::Disable,
            "prefer" => SslMode::Prefer,
            "require" => SslMode::Require,
            "verify-ca" => SslMode::VerifyCa,
            "verify-full" => SslMode::VerifyFull,
            _ => SslMode::Prefer, // Default fallback
        }
    }

    /// Convert this SSL mode to a database string
    pub fn to_db_str(&self) -> &'static str {
        match self {
            SslMode::Disable => "disable",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
            SslMode::VerifyCa => "verify-ca",
            SslMode::VerifyFull => "verify-full",
        }
    }
}

/// PostgreSQL connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub username: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub password: String,
    pub database: String,
    pub port: usize,
    #[serde(default)]
    pub ssl_mode: SslMode,
}

impl ConnectionInfo {
    /// Create a new connection info with the given parameters
    pub fn new(
        name: String,
        hostname: String,
        username: String,
        password: String,
        database: String,
        port: usize,
        ssl_mode: SslMode,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            hostname,
            username,
            password,
            database,
            port,
            ssl_mode,
        }
    }

    /// Create connection options for sqlx without exposing password
    pub fn to_pg_connect_options(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(&self.hostname)
            .port(self.port as u16)
            .username(&self.username)
            .password(&self.password)
            .database(&self.database)
            .ssl_mode(self.ssl_mode.to_pg_ssl_mode())
    }
}

impl Default for ConnectionInfo {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            hostname: "localhost".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
            database: "test".to_string(),
            port: 5432,
            ssl_mode: SslMode::default(),
        }
    }
}

impl Drop for ConnectionInfo {
    fn drop(&mut self) {
        // Zero out password memory when dropped for security
        use std::ptr;
        unsafe {
            ptr::write_volatile(&mut self.password, String::new());
        }
    }
}

/// Query history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub sql: String,
    pub execution_time_ms: i64,
    pub rows_affected: Option<i64>,
    pub success: bool,
    pub error_message: Option<String>,
    pub executed_at: DateTime<Utc>,
}
