//! Connection type definitions.
//!
//! This module contains:
//! - `DatabaseDriver` - which database backend a connection uses
//! - `SslMode` - SSL mode options (PostgreSQL semantics; mapped to MySQL too)
//! - `ConnectionInfo` - database connection configuration
use chrono::{DateTime, Utc};
use gpui::SharedString;
use gpui_component::select::SelectItem;
use serde::{Deserialize, Serialize};
use sqlx::mysql::{MySqlConnectOptions, MySqlSslMode};
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use uuid::Uuid;

use crate::services::ssh::SshConfig;

// ============================================================================
// DatabaseDriver
// ============================================================================

/// Which database backend a saved connection targets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseDriver {
    Postgres,
    MySql,
}

impl Default for DatabaseDriver {
    fn default() -> Self {
        DatabaseDriver::Postgres
    }
}

impl DatabaseDriver {
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseDriver::Postgres => "PostgreSQL",
            DatabaseDriver::MySql => "MySQL",
        }
    }

    pub fn to_db_str(&self) -> &'static str {
        match self {
            DatabaseDriver::Postgres => "postgres",
            DatabaseDriver::MySql => "mysql",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "mysql" => DatabaseDriver::MySql,
            _ => DatabaseDriver::Postgres,
        }
    }

    pub fn default_port(&self) -> usize {
        match self {
            DatabaseDriver::Postgres => 5432,
            DatabaseDriver::MySql => 3306,
        }
    }

    pub fn all() -> Vec<DatabaseDriver> {
        vec![DatabaseDriver::Postgres, DatabaseDriver::MySql]
    }

    #[allow(dead_code)]
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => DatabaseDriver::MySql,
            _ => DatabaseDriver::Postgres,
        }
    }

    pub fn to_index(&self) -> usize {
        match self {
            DatabaseDriver::Postgres => 0,
            DatabaseDriver::MySql => 1,
        }
    }
}

impl SelectItem for DatabaseDriver {
    type Value = &'static str;

    fn title(&self) -> SharedString {
        self.as_str().into()
    }

    fn value(&self) -> &Self::Value {
        match self {
            DatabaseDriver::Postgres => &"postgres",
            DatabaseDriver::MySql => &"mysql",
        }
    }
}

// ============================================================================
// SslMode
// ============================================================================

/// SSL mode options for database connections.
///
/// These names follow PostgreSQL conventions; for MySQL the variants map
/// to the closest equivalent (`Disable`/`Prefer` → `Disabled`/`Preferred`,
/// `Require`/`VerifyCa`/`VerifyFull` → `Required`/`VerifyCa`/`VerifyIdentity`).
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

    /// Convert to sqlx MySqlSslMode
    pub fn to_mysql_ssl_mode(&self) -> MySqlSslMode {
        match self {
            SslMode::Disable => MySqlSslMode::Disabled,
            SslMode::Prefer => MySqlSslMode::Preferred,
            SslMode::Require => MySqlSslMode::Required,
            SslMode::VerifyCa => MySqlSslMode::VerifyCa,
            SslMode::VerifyFull => MySqlSslMode::VerifyIdentity,
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

// ============================================================================
// ConnectionInfo
// ============================================================================

/// Database connection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub driver: DatabaseDriver,
    pub hostname: String,
    pub username: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub password: String,
    pub database: String,
    pub port: usize,
    #[serde(default)]
    pub ssl_mode: SslMode,
    /// Optional SSH tunnel. When `Some`, pgui will open the tunnel first
    /// and connect to the database through `127.0.0.1:<tunnel-port>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<SshConfig>,
}

impl ConnectionInfo {
    /// Create a new PostgreSQL connection info with the given parameters.
    #[allow(dead_code)]
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
            driver: DatabaseDriver::Postgres,
            hostname,
            username,
            password,
            database,
            port,
            ssl_mode,
            ssh: None,
        }
    }

    /// Create a Postgres `PgConnectOptions` for the given host/port pair.
    /// `host`/`port` may differ from `self.hostname`/`self.port` when an
    /// SSH tunnel is in use (caller passes the tunnel-local endpoint).
    pub fn to_pg_connect_options_for(&self, host: &str, port: u16) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(host)
            .port(port)
            .username(&self.username)
            .password(&self.password)
            .database(&self.database)
            .ssl_mode(self.ssl_mode.to_pg_ssl_mode())
    }

    /// Create a MySQL `MySqlConnectOptions` for the given host/port pair.
    pub fn to_mysql_connect_options_for(&self, host: &str, port: u16) -> MySqlConnectOptions {
        MySqlConnectOptions::new()
            .host(host)
            .port(port)
            .username(&self.username)
            .password(&self.password)
            .database(&self.database)
            .ssl_mode(self.ssl_mode.to_mysql_ssl_mode())
    }

    /// Direct-connection Postgres options (no SSH tunnel).
    #[allow(dead_code)]
    pub fn to_pg_connect_options(&self) -> PgConnectOptions {
        self.to_pg_connect_options_for(&self.hostname, self.port as u16)
    }

    /// Direct-connection MySQL options (no SSH tunnel).
    #[allow(dead_code)]
    pub fn to_mysql_connect_options(&self) -> MySqlConnectOptions {
        self.to_mysql_connect_options_for(&self.hostname, self.port as u16)
    }
}

impl Default for ConnectionInfo {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            driver: DatabaseDriver::Postgres,
            hostname: "localhost".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
            database: "test".to_string(),
            port: 5432,
            ssl_mode: SslMode::default(),
            ssh: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ssh::{SshAuth, SshConfig};

    // ====================================================================
    // DatabaseDriver
    // ====================================================================

    #[test]
    fn database_driver_db_str_roundtrip() {
        for d in DatabaseDriver::all() {
            assert_eq!(DatabaseDriver::from_db_str(d.to_db_str()), d);
        }
        // Unknown values fall back to Postgres for forward-compat.
        assert_eq!(
            DatabaseDriver::from_db_str("future-driver"),
            DatabaseDriver::Postgres
        );
        // Empty string -> Postgres (avoids panicking on legacy rows).
        assert_eq!(DatabaseDriver::from_db_str(""), DatabaseDriver::Postgres);
    }

    #[test]
    fn database_driver_index_roundtrip() {
        for d in DatabaseDriver::all() {
            assert_eq!(DatabaseDriver::from_index(d.to_index()), d);
        }
        // Out-of-range -> Postgres.
        assert_eq!(DatabaseDriver::from_index(99), DatabaseDriver::Postgres);
    }

    #[test]
    fn database_driver_default_ports() {
        assert_eq!(DatabaseDriver::Postgres.default_port(), 5432);
        assert_eq!(DatabaseDriver::MySql.default_port(), 3306);
    }

    #[test]
    fn database_driver_serde_roundtrip() {
        // The on-disk JSON form for the driver field should be lowercase.
        for d in DatabaseDriver::all() {
            let json = serde_json::to_string(&d).unwrap();
            let back: DatabaseDriver = serde_json::from_str(&json).unwrap();
            assert_eq!(d, back);
            // Confirm the rename_all="lowercase" attribute really did fire.
            assert!(
                json == "\"postgres\"" || json == "\"mysql\"",
                "unexpected driver json {}",
                json
            );
        }
    }

    #[test]
    fn database_driver_default_is_postgres() {
        assert_eq!(DatabaseDriver::default(), DatabaseDriver::Postgres);
    }

    #[test]
    fn database_driver_select_item_titles() {
        use gpui_component::select::SelectItem;
        assert_eq!(
            DatabaseDriver::Postgres.title().to_string(),
            "PostgreSQL"
        );
        assert_eq!(DatabaseDriver::MySql.title().to_string(), "MySQL");
    }

    // ====================================================================
    // SslMode
    // ====================================================================

    #[test]
    fn ssl_mode_db_str_roundtrip() {
        for m in SslMode::all() {
            assert_eq!(SslMode::from_db_str(m.to_db_str()), m);
        }
        // Unknown values fall back to Prefer.
        assert_eq!(SslMode::from_db_str("banana"), SslMode::Prefer);
    }

    #[test]
    fn ssl_mode_index_roundtrip() {
        for m in SslMode::all() {
            assert_eq!(SslMode::from_index(m.to_index()), m);
        }
        // Out-of-range -> Prefer.
        assert_eq!(SslMode::from_index(42), SslMode::Prefer);
    }

    #[test]
    fn ssl_mode_pg_mappings_complete() {
        // sqlx's SslMode types don't impl PartialEq, so compare via Debug.
        let cases = [
            (SslMode::Disable, "Disable"),
            (SslMode::Prefer, "Prefer"),
            (SslMode::Require, "Require"),
            (SslMode::VerifyCa, "VerifyCa"),
            (SslMode::VerifyFull, "VerifyFull"),
        ];
        for (m, expected) in cases {
            assert_eq!(format!("{:?}", m.to_pg_ssl_mode()), expected);
        }
    }

    #[test]
    fn ssl_mode_mysql_mappings_complete() {
        let cases = [
            (SslMode::Disable, "Disabled"),
            (SslMode::Prefer, "Preferred"),
            (SslMode::Require, "Required"),
            (SslMode::VerifyCa, "VerifyCa"),
            (SslMode::VerifyFull, "VerifyIdentity"),
        ];
        for (m, expected) in cases {
            assert_eq!(format!("{:?}", m.to_mysql_ssl_mode()), expected);
        }
    }

    // ====================================================================
    // SshConfig / SshAuth
    // ====================================================================

    #[test]
    fn ssh_config_serde_keyfile_roundtrip() {
        let cfg = SshConfig {
            host: "bastion.example.com".to_string(),
            port: 2222,
            username: "deploy".to_string(),
            auth: SshAuth::KeyFile {
                path: "/Users/me/.ssh/id_ed25519".to_string(),
            },
        };
        let json = serde_json::to_string(&cfg).unwrap();
        // Tagged enum with snake_case discriminator.
        assert!(json.contains("\"type\":\"key_file\""), "got {}", json);
        let back: SshConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn ssh_config_serde_agent_roundtrip() {
        let cfg = SshConfig {
            host: "h".to_string(),
            port: 22,
            username: "u".to_string(),
            auth: SshAuth::Agent,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"type\":\"agent\""), "got {}", json);
        let back: SshConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn ssh_auth_default_is_agent() {
        assert_eq!(SshAuth::default(), SshAuth::Agent);
    }

    #[test]
    fn ssh_auth_as_str() {
        assert_eq!(SshAuth::Agent.as_str(), "agent");
        assert_eq!(
            SshAuth::KeyFile {
                path: "/x".to_string()
            }
            .as_str(),
            "key_file"
        );
    }

    // ====================================================================
    // ConnectionInfo
    // ====================================================================

    #[test]
    fn connection_info_empty_password_is_skipped() {
        // After load_all() passwords are empty strings (loaded on-demand
        // from the keyring). Verify this round-trips without leaking a
        // bogus empty password key into the JSON.
        let mut info = ConnectionInfo::default();
        info.password = String::new();
        let json = serde_json::to_string(&info).unwrap();
        assert!(
            !json.contains("\"password\""),
            "unexpected password key: {}",
            json
        );
    }

    #[test]
    fn connection_info_default_has_no_ssh() {
        let info = ConnectionInfo::default();
        assert!(info.ssh.is_none());
        assert_eq!(info.driver, DatabaseDriver::Postgres);
    }

    #[test]
    fn connection_info_ssh_skipped_when_none() {
        let info = ConnectionInfo::default();
        let json = serde_json::to_string(&info).unwrap();
        assert!(
            !json.contains("\"ssh\""),
            "ssh should be skip_serializing_if=None: {}",
            json
        );
    }

    #[test]
    fn connection_info_with_ssh_serde_roundtrip() {
        let mut info = ConnectionInfo::default();
        info.password = String::new(); // skipped by serde
        info.driver = DatabaseDriver::MySql;
        info.port = 3306;
        info.ssh = Some(SshConfig {
            host: "jump.example.com".to_string(),
            port: 22,
            username: "ops".to_string(),
            auth: SshAuth::KeyFile {
                path: "/home/ops/.ssh/id_rsa".to_string(),
            },
        });
        let json = serde_json::to_string(&info).unwrap();
        let back: ConnectionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.driver, DatabaseDriver::MySql);
        assert_eq!(back.port, 3306);
        assert_eq!(back.ssh, info.ssh);
    }

    #[test]
    fn connection_info_legacy_json_without_driver_or_ssh() {
        // Older saved blobs (pre-MySQL/SSH) deserialize cleanly with
        // serde defaults filling in the new fields.
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "old",
            "hostname": "db",
            "username": "u",
            "database": "d",
            "port": 5432
        }"#;
        let info: ConnectionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.driver, DatabaseDriver::Postgres);
        assert!(info.ssh.is_none());
        assert_eq!(info.ssl_mode, SslMode::Prefer);
    }

    #[test]
    fn pg_connect_options_use_overridden_host_port() {
        // When a tunnel is in use we connect via 127.0.0.1:<random>;
        // make sure the override knobs actually substitute that endpoint.
        let mut info = ConnectionInfo::default();
        info.hostname = "db.internal".to_string();
        info.port = 5432;
        let opts = info.to_pg_connect_options_for("127.0.0.1", 49152);
        // sqlx exposes host()/port() on PgConnectOptions in 0.8.
        assert_eq!(opts.get_host(), "127.0.0.1");
        assert_eq!(opts.get_port(), 49152);
    }

    #[test]
    fn mysql_connect_options_use_overridden_host_port() {
        let mut info = ConnectionInfo::default();
        info.driver = DatabaseDriver::MySql;
        info.hostname = "mysql.internal".to_string();
        info.port = 3306;
        info.username = "app".to_string();
        info.database = "appdb".to_string();
        let opts = info.to_mysql_connect_options_for("127.0.0.1", 50001);
        // MySqlConnectOptions also exposes get_host/get_port in sqlx 0.8.
        assert_eq!(opts.get_host(), "127.0.0.1");
        assert_eq!(opts.get_port(), 50001);
    }

    #[test]
    fn pg_connect_options_carry_credentials_and_database() {
        let mut info = ConnectionInfo::default();
        info.username = "alice".to_string();
        info.database = "appdb".to_string();
        info.password = "secret".to_string();
        let opts = info.to_pg_connect_options_for("db", 5432);
        assert_eq!(opts.get_username(), "alice");
        assert_eq!(opts.get_database(), Some("appdb"));
        // Password is not exposed on get_* (good); we only assert
        // construction didn't panic and the rest of the fields are right.
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
