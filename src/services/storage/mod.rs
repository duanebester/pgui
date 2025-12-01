//! Unified SQLite storage for the application.

mod connections;
mod history;
mod types;

pub use connections::ConnectionsRepository;
pub use history::QueryHistoryRepository;
pub use types::*;

use anyhow::Result;
use async_lock::OnceCell;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;

/// Shared application storage backed by SQLite.
#[derive(Debug, Clone)]
pub struct AppStore {
    pool: SqlitePool,
}

/// Global singleton instance
static STORE: OnceCell<AppStore> = OnceCell::new();

impl AppStore {
    /// Get or initialize the global AppStore singleton.
    /// Schema initialization and migration only run once.
    pub async fn singleton() -> Result<&'static Self> {
        STORE.get_or_try_init(|| Self::init()).await
    }

    pub async fn init() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::from_path(db_path).await
    }

    async fn from_path(db_path: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

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

    fn get_db_path() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(".pgui").join("pgui.db")) // Renamed to be more generic
    }

    /// Get a connections repository
    pub fn connections(&self) -> ConnectionsRepository {
        ConnectionsRepository::new(self.pool.clone())
    }

    /// Get a query history repository
    #[allow(dead_code)]
    pub fn history(&self) -> QueryHistoryRepository {
        QueryHistoryRepository::new(self.pool.clone())
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

        // Query history table
        sqlx::query(
            r#"
                CREATE TABLE IF NOT EXISTS query_history (
                    id TEXT PRIMARY KEY,
                    connection_id TEXT NOT NULL,
                    sql TEXT NOT NULL,
                    execution_time_ms INTEGER NOT NULL,
                    rows_affected INTEGER,
                    success INTEGER NOT NULL,
                    error_message TEXT,
                    executed_at TIMESTAMP NOT NULL,
                    FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE CASCADE
                )
                "#,
        )
        .execute(&self.pool)
        .await?;

        // Index for fast lookups by connection
        sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_history_connection ON query_history(connection_id, executed_at DESC)"
            )
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
}
