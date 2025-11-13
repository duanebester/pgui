use anyhow::{Context, Result};
use keyring::Entry;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

use super::{ConnectionInfo, SslMode, load_connections as load_json_connections};

const KEYRING_SERVICE: &str = "pgui";

/// ConnectionsStore manages the SQLite database for storing saved PostgreSQL connections
#[derive(Debug, Clone)]
pub struct ConnectionsStore {
    pool: SqlitePool,
}

#[allow(dead_code)]
impl ConnectionsStore {
    /// Create a new ConnectionsStore and initialize the database
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::from_path(db_path).await
    }

    /// Create a ConnectionsStore from a specific path (useful for testing)
    async fn from_path(db_path: PathBuf) -> Result<Self> {
        // Ensure the directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Use SqliteConnectOptions to set create_if_missing
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let store = Self { pool };
        store.initialize_schema().await?;
        store.migrate_schema().await?;
        Ok(store)
    }

    /// Get the path to the SQLite database file
    fn get_db_path() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(".pgui").join("connections.db"))
    }

    /// Initialize the database schema
    async fn initialize_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
                CREATE TABLE IF NOT EXISTS connections (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    hostname TEXT NOT NULL,
                    username TEXT NOT NULL,
                    database TEXT NOT NULL,
                    port INTEGER NOT NULL,
                    ssl_mode TEXT NOT NULL DEFAULT 'prefer',
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
                "#,
        )
        .execute(&self.pool)
        .await?;

        // Create index on name for faster lookups
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_connections_name ON connections(name)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Migrate schema for existing databases
    async fn migrate_schema(&self) -> Result<()> {
        // Try to check if ssl_mode column exists by querying a single row
        let has_ssl_mode = sqlx::query("SELECT ssl_mode FROM connections LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .is_ok();

        if !has_ssl_mode {
            println!("Migration: ssl_mode column not found, adding it...");

            // Add ssl_mode column with default value
            match sqlx::query(
                "ALTER TABLE connections ADD COLUMN ssl_mode TEXT NOT NULL DEFAULT 'prefer'",
            )
            .execute(&self.pool)
            .await
            {
                Ok(_) => {
                    println!("Migration: Successfully added ssl_mode column");
                }
                Err(e) => {
                    // If column already exists, SQLite will error - that's okay
                    println!("Migration: Column may already exist: {}", e);
                }
            }
        } else {
            println!("Migration: ssl_mode column already exists");
        }

        Ok(())
    }

    /// Get keyring entry for a connection
    fn get_keyring_entry(connection_id: &Uuid) -> Result<Entry> {
        Entry::new(KEYRING_SERVICE, &connection_id.to_string())
            .context("Failed to create keyring entry")
    }

    /// Store password in keyring
    fn store_password(connection_id: &Uuid, password: &str) -> Result<()> {
        let entry = Self::get_keyring_entry(connection_id)?;
        entry
            .set_password(password)
            .context("Failed to store password in keyring")
    }

    /// Retrieve password from keyring
    fn get_password(connection_id: &Uuid) -> Result<String> {
        let entry = Self::get_keyring_entry(connection_id)?;
        entry
            .get_password()
            .context("Failed to retrieve password from keyring")
    }

    /// Delete password from keyring
    fn delete_password(connection_id: &Uuid) -> Result<()> {
        let entry = Self::get_keyring_entry(connection_id)?;
        // Ignore errors if password doesn't exist
        let _ = entry.delete_credential();
        Ok(())
    }

    /// Parse SSL mode string from database
    fn parse_ssl_mode(ssl_mode_str: &str) -> SslMode {
        match ssl_mode_str {
            "disable" => SslMode::Disable,
            "prefer" => SslMode::Prefer,
            "require" => SslMode::Require,
            "verify-ca" => SslMode::VerifyCa,
            "verify-full" => SslMode::VerifyFull,
            _ => SslMode::Prefer, // Default fallback
        }
    }

    /// Convert SSL mode to database string
    fn ssl_mode_to_string(ssl_mode: &SslMode) -> &'static str {
        match ssl_mode {
            SslMode::Disable => "disable",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
            SslMode::VerifyCa => "verify-ca",
            SslMode::VerifyFull => "verify-full",
        }
    }

    /// Load all saved connections from the database
    pub async fn load_connections(&self) -> Result<Vec<ConnectionInfo>> {
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

            // DON'T load password during startup to avoid multiple keychain prompts
            // Password will be loaded on-demand when connecting
            let password = String::new();

            connections.push(ConnectionInfo {
                id,
                name,
                hostname,
                username,
                password,
                database,
                port: port as usize,
                ssl_mode: Self::parse_ssl_mode(&ssl_mode_str),
            });
        }

        Ok(connections)
    }

    /// Create a new connection
    pub async fn create_connection(&self, connection: &ConnectionInfo) -> Result<()> {
        // Check if a connection with the same name already exists
        if self.connection_exists_by_name(&connection.name).await? {
            anyhow::bail!(
                "A connection with the name '{}' already exists",
                connection.name
            );
        }

        // Store password in keyring first
        if !connection.password.is_empty() {
            Self::store_password(&connection.id, &connection.password)?;
        }

        // Insert connection metadata in SQLite (without password)
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
        .bind(Self::ssl_mode_to_string(&connection.ssl_mode))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update an existing connection
    pub async fn update_connection(&self, connection: &ConnectionInfo) -> Result<()> {
        // Check if a different connection with the same name exists
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

        // Update password in keyring
        if !connection.password.is_empty() {
            Self::store_password(&connection.id, &connection.password)?;
        }

        // Update connection metadata in SQLite (without password)
        sqlx::query(
            r#"
                UPDATE connections
                SET name = ?2,
                    hostname = ?3,
                    username = ?4,
                    database = ?5,
                    port = ?6,
                    ssl_mode = ?7,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?1
                "#,
        )
        .bind(connection.id.to_string())
        .bind(&connection.name)
        .bind(&connection.hostname)
        .bind(&connection.username)
        .bind(&connection.database)
        .bind(connection.port as i64)
        .bind(Self::ssl_mode_to_string(&connection.ssl_mode))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a connection by ID
    pub async fn delete_connection(&self, id: &Uuid) -> Result<()> {
        // Delete password from keyring
        Self::delete_password(id)?;

        // Delete from database
        sqlx::query("DELETE FROM connections WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get a single connection by ID
    pub async fn get_connection(&self, id: &Uuid) -> Result<Option<ConnectionInfo>> {
        let result = sqlx::query_as::<_, (String, String, String, String, String, i64, String)>(
            "SELECT id, name, hostname, username, database, port, ssl_mode
                 FROM connections
                 WHERE id = ?1",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(
            |(id_str, name, hostname, username, database, port, ssl_mode_str)| {
                let id = Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4());
                // DON'T load password here either - let caller load it on-demand
                let password = String::new();

                ConnectionInfo {
                    id,
                    name,
                    hostname,
                    username,
                    password,
                    database,
                    port: port as usize,
                    ssl_mode: Self::parse_ssl_mode(&ssl_mode_str),
                }
            },
        ))
    }

    /// Get password for a specific connection from keyring
    /// This should be called on-demand when actually connecting to avoid
    /// multiple keychain access prompts on startup
    pub fn get_connection_password(connection_id: &Uuid) -> Result<String> {
        Self::get_password(connection_id)
    }

    /// Check if a connection with the given name exists
    pub async fn connection_exists_by_name(&self, name: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM connections WHERE name = ?1")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;

        Ok(count > 0)
    }

    /// Migrate existing connections from JSON file to SQLite
    /// This is a one-time migration helper
    pub async fn migrate_from_json(&self) -> Result<usize> {
        let connections: Vec<ConnectionInfo> = load_json_connections().await;
        let count = connections.len();

        for connection in connections {
            // Only save if it doesn't already exist in the database
            if !self.connection_exists_by_name(&connection.name).await? {
                self.create_connection(&connection).await?;
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_std::test]
    async fn test_new_store_creates_schema() {
        // Use an in-memory database for tests
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .unwrap();

        let store = ConnectionsStore { pool };
        store.initialize_schema().await.unwrap();

        // If we got here, the store was created successfully
        assert!(store.load_connections().await.is_ok());
    }

    #[async_std::test]
    async fn test_save_and_load_connection() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .unwrap();

        let store = ConnectionsStore { pool };
        store.initialize_schema().await.unwrap();

        let connection = ConnectionInfo {
            id: Uuid::new_v4(),
            name: "test-connection".to_string(),
            hostname: "localhost".to_string(),
            username: "testuser".to_string(),
            password: "testpass".to_string(),
            database: "testdb".to_string(),
            port: 5432,
            ssl_mode: SslMode::Prefer,
        };

        let conn_id = connection.id;

        // Save connection
        store.create_connection(&connection).await.unwrap();

        // Load connections
        let connections = store.load_connections().await.unwrap();
        assert!(connections.iter().any(|c| c.id == conn_id));

        // Get specific connection
        let loaded = store.get_connection(&conn_id).await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.hostname, "localhost");
        assert_eq!(loaded.port, 5432);
        assert_eq!(loaded.ssl_mode, SslMode::Prefer);
    }

    #[async_std::test]
    async fn test_update_connection() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .unwrap();

        let store = ConnectionsStore { pool };
        store.initialize_schema().await.unwrap();

        let mut connection = ConnectionInfo {
            id: Uuid::new_v4(),
            name: "update-test".to_string(),
            hostname: "localhost".to_string(),
            username: "user1".to_string(),
            password: "pass1".to_string(),
            database: "db1".to_string(),
            port: 5432,
            ssl_mode: SslMode::Disable,
        };

        let conn_id = connection.id;

        // Save initial
        store.create_connection(&connection).await.unwrap();

        // Update
        connection.hostname = "newhost".to_string();
        connection.port = 5433;
        connection.ssl_mode = SslMode::Require;
        store.update_connection(&connection).await.unwrap();

        // Verify update
        let loaded = store.get_connection(&conn_id).await.unwrap().unwrap();
        assert_eq!(loaded.hostname, "newhost");
        assert_eq!(loaded.port, 5433);
        assert_eq!(loaded.ssl_mode, SslMode::Require);
    }

    #[async_std::test]
    async fn test_delete_connection() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .unwrap();

        let store = ConnectionsStore { pool };
        store.initialize_schema().await.unwrap();

        let connection = ConnectionInfo {
            id: Uuid::new_v4(),
            name: "delete-test".to_string(),
            hostname: "localhost".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            database: "db".to_string(),
            port: 5432,
            ssl_mode: SslMode::Prefer,
        };

        let conn_id = connection.id;

        // Save and verify exists
        store.create_connection(&connection).await.unwrap();
        assert!(store.get_connection(&conn_id).await.unwrap().is_some());

        // Delete and verify removed
        store.delete_connection(&conn_id).await.unwrap();
        assert!(store.get_connection(&conn_id).await.unwrap().is_none());
    }
}
