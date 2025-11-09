use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;

use super::{ConnectionInfo, load_connections as load_json_connections};

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
        Ok(store)
    }

    /// Get the path to the SQLite database file
    fn get_db_path() -> Result<PathBuf> {
        let home =
            std::env::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(".pgui").join("connections.db"))
    }

    /// Initialize the database schema
    async fn initialize_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS connections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                database TEXT NOT NULL,
                port INTEGER NOT NULL,
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

    /// Load all saved connections from the database
    pub async fn load_connections(&self) -> Result<Vec<ConnectionInfo>> {
        let connections = sqlx::query_as::<_, (String, String, String, String, String, i64)>(
            "SELECT name, hostname, username, password, database, port
             FROM connections
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(
            |(name, hostname, username, password, database, port)| ConnectionInfo {
                name,
                hostname,
                username,
                password,
                database,
                port: port as usize,
            },
        )
        .collect();

        Ok(connections)
    }

    /// Save a new connection or update an existing one
    pub async fn save_connection(&self, connection: &ConnectionInfo) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO connections (name, hostname, username, password, database, port, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP)
            ON CONFLICT(name) DO UPDATE SET
                hostname = excluded.hostname,
                username = excluded.username,
                password = excluded.password,
                database = excluded.database,
                port = excluded.port,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&connection.name)
        .bind(&connection.hostname)
        .bind(&connection.username)
        .bind(&connection.password)
        .bind(&connection.database)
        .bind(connection.port as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a connection by name
    pub async fn delete_connection(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM connections WHERE name = ?1")
            .bind(name)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get a single connection by name
    pub async fn get_connection(&self, name: &str) -> Result<Option<ConnectionInfo>> {
        let result = sqlx::query_as::<_, (String, String, String, String, String, i64)>(
            "SELECT name, hostname, username, password, database, port
             FROM connections
             WHERE name = ?1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(
            |(name, hostname, username, password, database, port)| ConnectionInfo {
                name,
                hostname,
                username,
                password,
                database,
                port: port as usize,
            },
        ))
    }

    /// Check if a connection with the given name exists
    pub async fn connection_exists(&self, name: &str) -> Result<bool> {
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
            if !self.connection_exists(&connection.name).await? {
                self.save_connection(&connection).await?;
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
            name: "test-connection".to_string(),
            hostname: "localhost".to_string(),
            username: "testuser".to_string(),
            password: "testpass".to_string(),
            database: "testdb".to_string(),
            port: 5432,
        };

        // Save connection
        store.save_connection(&connection).await.unwrap();

        // Load connections
        let connections = store.load_connections().await.unwrap();
        assert!(connections.iter().any(|c| c.name == "test-connection"));

        // Get specific connection
        let loaded = store.get_connection("test-connection").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.hostname, "localhost");
        assert_eq!(loaded.port, 5432);
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
            name: "update-test".to_string(),
            hostname: "localhost".to_string(),
            username: "user1".to_string(),
            password: "pass1".to_string(),
            database: "db1".to_string(),
            port: 5432,
        };

        // Save initial
        store.save_connection(&connection).await.unwrap();

        // Update
        connection.hostname = "newhost".to_string();
        connection.port = 5433;
        store.save_connection(&connection).await.unwrap();

        // Verify update
        let loaded = store.get_connection("update-test").await.unwrap().unwrap();
        assert_eq!(loaded.hostname, "newhost");
        assert_eq!(loaded.port, 5433);
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
            name: "delete-test".to_string(),
            hostname: "localhost".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            database: "db".to_string(),
            port: 5432,
        };

        // Save and verify exists
        store.save_connection(&connection).await.unwrap();
        assert!(store.connection_exists("delete-test").await.unwrap());

        // Delete and verify removed
        store.delete_connection("delete-test").await.unwrap();
        assert!(!store.connection_exists("delete-test").await.unwrap());
    }

    #[async_std::test]
    async fn test_connection_exists() {
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
            name: "exists-test".to_string(),
            hostname: "localhost".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            database: "db".to_string(),
            port: 5432,
        };

        // Should not exist initially
        assert!(!store.connection_exists("exists-test").await.unwrap());

        // Save and verify exists
        store.save_connection(&connection).await.unwrap();
        assert!(store.connection_exists("exists-test").await.unwrap());
    }
}
