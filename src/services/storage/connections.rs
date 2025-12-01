//! Connection repository using SQLite and system keyring.

use anyhow::{Context, Result};
use keyring::Entry;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::types::{ConnectionInfo, SslMode};

const KEYRING_SERVICE: &str = "pgui";

/// Repository for connection CRUD operations.
///
/// Passwords are stored securely in the system keyring, while connection
/// metadata (host, port, username, etc.) is stored in SQLite.
#[derive(Debug, Clone)]
pub struct ConnectionsRepository {
    pool: SqlitePool,
}

impl ConnectionsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ========== Keyring Methods ==========

    fn get_keyring_entry(connection_id: &Uuid) -> Result<Entry> {
        Entry::new(KEYRING_SERVICE, &connection_id.to_string())
            .context("Failed to create keyring entry")
    }

    fn store_password(connection_id: &Uuid, password: &str) -> Result<()> {
        let entry = Self::get_keyring_entry(connection_id)?;
        entry
            .set_password(password)
            .context("Failed to store password in keyring")
    }

    fn get_password(connection_id: &Uuid) -> Result<String> {
        let entry = Self::get_keyring_entry(connection_id)?;
        entry
            .get_password()
            .context("Failed to retrieve password from keyring")
    }

    fn delete_password(connection_id: &Uuid) -> Result<()> {
        let entry = Self::get_keyring_entry(connection_id)?;
        let _ = entry.delete_credential();
        Ok(())
    }

    // ========== CRUD Methods ==========

    /// Load all saved connections from the database
    pub async fn load_all(&self) -> Result<Vec<ConnectionInfo>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, String)>(
            "SELECT id, name, hostname, username, database, port, ssl_mode
             FROM connections
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut connections = Vec::new();
        for (id_str, name, hostname, username, database, port, ssl_mode_str) in rows {
            let id = Uuid::parse_str(&id_str).context("Invalid UUID in database")?;
            let password = String::new(); // Load on-demand to avoid keychain prompts

            connections.push(ConnectionInfo {
                id,
                name,
                hostname,
                username,
                password,
                database,
                port: port as usize,
                ssl_mode: SslMode::from_db_str(&ssl_mode_str),
            });
        }

        Ok(connections)
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

        sqlx::query(
            r#"
            INSERT INTO connections (id, name, hostname, username, database, port, ssl_mode, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(connection.id.to_string())
        .bind(&connection.name)
        .bind(&connection.hostname)
        .bind(&connection.username)
        .bind(&connection.database)
        .bind(connection.port as i64)
        .bind(connection.ssl_mode.to_db_str())
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

        sqlx::query(
            r#"
            UPDATE connections
            SET name = ?2, hostname = ?3, username = ?4, database = ?5,
                port = ?6, ssl_mode = ?7, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
        )
        .bind(connection.id.to_string())
        .bind(&connection.name)
        .bind(&connection.hostname)
        .bind(&connection.username)
        .bind(&connection.database)
        .bind(connection.port as i64)
        .bind(connection.ssl_mode.to_db_str())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a connection by ID
    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        Self::delete_password(id)?;
        sqlx::query("DELETE FROM connections WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get a single connection by ID
    #[allow(dead_code)]
    pub async fn get(&self, id: &Uuid) -> Result<Option<ConnectionInfo>> {
        let result = sqlx::query_as::<_, (String, String, String, String, String, i64, String)>(
            "SELECT id, name, hostname, username, database, port, ssl_mode
             FROM connections WHERE id = ?1",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(
            |(id_str, name, hostname, username, database, port, ssl_mode_str)| ConnectionInfo {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                name,
                hostname,
                username,
                password: String::new(),
                database,
                port: port as usize,
                ssl_mode: SslMode::from_db_str(&ssl_mode_str),
            },
        ))
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
