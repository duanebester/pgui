use anyhow::Result;
use async_lock::RwLock;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DatabaseManager {
    pub(crate) pool: Arc<RwLock<Option<PgPool>>>,
}

impl DatabaseManager {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn connect_with_options(&self, options: PgConnectOptions) -> Result<()> {
        let pool_opts = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5));

        let pool = pool_opts.connect_with(options).await;

        match pool {
            Ok(p) => {
                let mut pool_guard = self.pool.write().await;
                *pool_guard = Some(p);
            }
            Err(e) => {
                tracing::error!("Error Connecting: {}", e)
            }
        };

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn test_connection_options(options: PgConnectOptions) -> Result<()> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await?;

        sqlx::query("SELECT 1").fetch_one(&pool).await?;
        pool.close().await;

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        let mut pool_guard = self.pool.write().await;
        if let Some(pool) = pool_guard.take() {
            pool.close().await;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "No active database connection to disconnect"
            ))
        }
    }

    pub async fn is_connected(&self) -> bool {
        let pool_guard = self.pool.read().await;
        if let Some(pool) = pool_guard.as_ref() {
            sqlx::query("SELECT 1").fetch_one(pool).await.is_ok()
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub async fn test_connection(&self) -> Result<bool> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let _: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pool).await?;
        Ok(true)
    }
}
