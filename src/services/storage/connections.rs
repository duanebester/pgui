//! Connection repository using SQLite and system keyring.
//!
//! Layout of secrets in the system keyring (service `pgui`):
//! - `<connection-id>`              -> database password
//! - `<connection-id>:ssh-keypass`  -> SSH private-key passphrase (optional)

use anyhow::{Context, Result};
use keyring::Entry;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::types::{ConnectionInfo, DatabaseDriver, SslMode};
use crate::services::ssh::{SshAuth, SshConfig};

const KEYRING_SERVICE: &str = "pgui";
const SSH_KEYPASS_SUFFIX: &str = ":ssh-keypass";

/// Repository for connection CRUD operations.
///
/// Passwords are stored securely in the system keyring, while connection
/// metadata (host, port, username, etc.) is stored in SQLite.
#[derive(Debug, Clone)]
pub struct ConnectionsRepository {
    pool: SqlitePool,
}

// Tuple of every column returned by SELECT statements below. Kept as a
// type alias so the `(...)` is in one place.
type ConnRow = (
    String,         // id
    String,         // name
    String,         // driver
    String,         // hostname
    String,         // username
    String,         // database
    i64,            // port
    String,         // ssl_mode
    i64,            // ssh_enabled
    Option<String>, // ssh_host
    Option<i64>,    // ssh_port
    Option<String>, // ssh_username
    Option<String>, // ssh_auth_type
    Option<String>, // ssh_key_path
);

const SELECT_COLS: &str = "id, name, driver, hostname, username, database, port, ssl_mode, \
     ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_auth_type, ssh_key_path";

impl ConnectionsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ========== Keyring Methods ==========

    fn keyring_entry(key: &str) -> Result<Entry> {
        Entry::new(KEYRING_SERVICE, key).context("Failed to create keyring entry")
    }

    fn store_password(connection_id: &Uuid, password: &str) -> Result<()> {
        let entry = Self::keyring_entry(&connection_id.to_string())?;
        entry
            .set_password(password)
            .context("Failed to store password in keyring")
    }

    fn get_password(connection_id: &Uuid) -> Result<String> {
        let entry = Self::keyring_entry(&connection_id.to_string())?;
        entry
            .get_password()
            .context("Failed to retrieve password from keyring")
    }

    fn delete_password(connection_id: &Uuid) -> Result<()> {
        let entry = Self::keyring_entry(&connection_id.to_string())?;
        let _ = entry.delete_credential();
        Ok(())
    }

    fn ssh_keypass_key(connection_id: &Uuid) -> String {
        format!("{}{}", connection_id, SSH_KEYPASS_SUFFIX)
    }

    /// Store an SSH key passphrase for a connection. Pass an empty string
    /// to clear it.
    pub fn store_ssh_key_passphrase(connection_id: &Uuid, passphrase: &str) -> Result<()> {
        let entry = Self::keyring_entry(&Self::ssh_keypass_key(connection_id))?;
        if passphrase.is_empty() {
            let _ = entry.delete_credential();
            Ok(())
        } else {
            entry
                .set_password(passphrase)
                .context("Failed to store SSH key passphrase in keyring")
        }
    }

    /// Retrieve an SSH key passphrase for a connection, if one is stored.
    pub fn get_ssh_key_passphrase(connection_id: &Uuid) -> Option<String> {
        let entry = Self::keyring_entry(&Self::ssh_keypass_key(connection_id)).ok()?;
        entry.get_password().ok()
    }

    fn delete_ssh_key_passphrase(connection_id: &Uuid) {
        if let Ok(entry) = Self::keyring_entry(&Self::ssh_keypass_key(connection_id)) {
            let _ = entry.delete_credential();
        }
    }

    // ========== Mapping Helpers ==========

    fn row_to_info(row: ConnRow) -> Result<ConnectionInfo> {
        let (
            id_str,
            name,
            driver_str,
            hostname,
            username,
            database,
            port,
            ssl_mode_str,
            ssh_enabled,
            ssh_host,
            ssh_port,
            ssh_username,
            ssh_auth_type,
            ssh_key_path,
        ) = row;

        let id = Uuid::parse_str(&id_str).context("Invalid UUID in database")?;

        let ssh = if ssh_enabled != 0 {
            let auth = match ssh_auth_type.as_deref() {
                Some("key_file") => SshAuth::KeyFile {
                    path: ssh_key_path.unwrap_or_default(),
                },
                _ => SshAuth::Agent,
            };
            Some(SshConfig {
                host: ssh_host.unwrap_or_default(),
                port: ssh_port.unwrap_or(22) as u16,
                username: ssh_username.unwrap_or_default(),
                auth,
            })
        } else {
            None
        };

        Ok(ConnectionInfo {
            id,
            name,
            driver: DatabaseDriver::from_db_str(&driver_str),
            hostname,
            username,
            password: String::new(), // load on demand
            database,
            port: port as usize,
            ssl_mode: SslMode::from_db_str(&ssl_mode_str),
            ssh,
        })
    }

    fn ssh_fields_for_write(
        ssh: &Option<SshConfig>,
    ) -> (i64, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>) {
        match ssh {
            None => (0, None, None, None, None, None),
            Some(cfg) => {
                let (auth_type, key_path) = match &cfg.auth {
                    SshAuth::KeyFile { path } => (
                        Some("key_file".to_string()),
                        Some(path.clone()),
                    ),
                    SshAuth::Agent => (Some("agent".to_string()), None),
                };
                (
                    1,
                    Some(cfg.host.clone()),
                    Some(cfg.port as i64),
                    Some(cfg.username.clone()),
                    auth_type,
                    key_path,
                )
            }
        }
    }

    // ========== CRUD Methods ==========

    /// Load all saved connections from the database
    pub async fn load_all(&self) -> Result<Vec<ConnectionInfo>> {
        let sql = format!(
            "SELECT {} FROM connections ORDER BY name",
            SELECT_COLS
        );
        let rows = sqlx::query_as::<_, ConnRow>(&sql)
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter().map(Self::row_to_info).collect()
    }

    /// Create a new connection
    pub async fn create(&self, connection: &ConnectionInfo) -> Result<()> {
        if self.exists_by_name(&connection.name).await? {
            anyhow::bail!(
                "A connection with the name '{}' already exists",
                connection.name
            );
        }

        if !connection.password.is_empty() {
            Self::store_password(&connection.id, &connection.password)?;
        }

        let (
            ssh_enabled,
            ssh_host,
            ssh_port,
            ssh_user,
            ssh_auth_type,
            ssh_key_path,
        ) = Self::ssh_fields_for_write(&connection.ssh);

        sqlx::query(
            r#"
            INSERT INTO connections (
                id, name, driver, hostname, username, database, port, ssl_mode,
                ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_auth_type, ssh_key_path,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(connection.id.to_string())
        .bind(&connection.name)
        .bind(connection.driver.to_db_str())
        .bind(&connection.hostname)
        .bind(&connection.username)
        .bind(&connection.database)
        .bind(connection.port as i64)
        .bind(connection.ssl_mode.to_db_str())
        .bind(ssh_enabled)
        .bind(ssh_host)
        .bind(ssh_port)
        .bind(ssh_user)
        .bind(ssh_auth_type)
        .bind(ssh_key_path)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update an existing connection
    pub async fn update(&self, connection: &ConnectionInfo) -> Result<()> {
        let existing = sqlx::query_scalar::<_, String>(
            "SELECT id FROM connections WHERE name = ?1 AND id != ?2",
        )
        .bind(&connection.name)
        .bind(connection.id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        if existing.is_some() {
            anyhow::bail!(
                "A connection with the name '{}' already exists",
                connection.name
            );
        }

        if !connection.password.is_empty() {
            Self::store_password(&connection.id, &connection.password)?;
        }

        let (
            ssh_enabled,
            ssh_host,
            ssh_port,
            ssh_user,
            ssh_auth_type,
            ssh_key_path,
        ) = Self::ssh_fields_for_write(&connection.ssh);

        sqlx::query(
            r#"
            UPDATE connections
            SET name = ?2, driver = ?3, hostname = ?4, username = ?5, database = ?6,
                port = ?7, ssl_mode = ?8,
                ssh_enabled = ?9, ssh_host = ?10, ssh_port = ?11,
                ssh_username = ?12, ssh_auth_type = ?13, ssh_key_path = ?14,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
        )
        .bind(connection.id.to_string())
        .bind(&connection.name)
        .bind(connection.driver.to_db_str())
        .bind(&connection.hostname)
        .bind(&connection.username)
        .bind(&connection.database)
        .bind(connection.port as i64)
        .bind(connection.ssl_mode.to_db_str())
        .bind(ssh_enabled)
        .bind(ssh_host)
        .bind(ssh_port)
        .bind(ssh_user)
        .bind(ssh_auth_type)
        .bind(ssh_key_path)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a connection by ID
    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        Self::delete_password(id)?;
        Self::delete_ssh_key_passphrase(id);
        sqlx::query("DELETE FROM connections WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get a single connection by ID
    #[allow(dead_code)]
    pub async fn get(&self, id: &Uuid) -> Result<Option<ConnectionInfo>> {
        let sql = format!(
            "SELECT {} FROM connections WHERE id = ?1",
            SELECT_COLS
        );
        let result = sqlx::query_as::<_, ConnRow>(&sql)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match result {
            Some(row) => Ok(Some(Self::row_to_info(row)?)),
            None => Ok(None),
        }
    }

    /// Get password for a connection from keyring (on-demand)
    pub fn get_connection_password(connection_id: &Uuid) -> Result<String> {
        Self::get_password(connection_id)
    }

    /// Check if a connection with the given name exists
    pub async fn exists_by_name(&self, name: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM connections WHERE name = ?1")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }
}
