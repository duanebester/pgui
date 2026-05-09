use anyhow::{anyhow, Result};
use async_lock::RwLock;
use futures::stream::BoxStream;
use futures::StreamExt;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use std::sync::Arc;
use std::time::Duration;

use super::mysql as my_backend;
use super::postgres as pg_backend;
use super::types::{
    DatabaseInfo, DatabaseSchema, ErrorResult, QueryExecutionResult, TableInfo,
};
use crate::services::ssh::SshTunnel;
use crate::services::storage::{ConnectionInfo, ConnectionsRepository, DatabaseDriver};

/// A live connection pool. Variant matches the backing database engine.
pub(crate) enum Pool {
    Postgres(PgPool),
    MySql(MySqlPool),
}

impl Pool {
    async fn close(self) {
        match self {
            Pool::Postgres(p) => p.close().await,
            Pool::MySql(p) => p.close().await,
        }
    }
}

/// Front-door for all database operations.
///
/// `DatabaseManager` is cheap to clone — internally it shares an
/// `Arc<RwLock<...>>` with the active pool and an optional SSH tunnel
/// that must outlive the pool.
#[derive(Clone)]
pub struct DatabaseManager {
    pub(crate) pool: Arc<RwLock<Option<Pool>>>,
    /// Held to keep the tunnel alive for the duration of the connection.
    /// Dropped on `disconnect()`.
    tunnel: Arc<RwLock<Option<SshTunnel>>>,
}

impl std::fmt::Debug for DatabaseManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseManager").finish()
    }
}

impl DatabaseManager {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(RwLock::new(None)),
            tunnel: Arc::new(RwLock::new(None)),
        }
    }

    /// Connect using a saved [`ConnectionInfo`].
    ///
    /// If `info.ssh` is set, opens an SSH tunnel first and then connects
    /// through `127.0.0.1:<tunnel-port>`. The tunnel is stored alongside
    /// the pool and torn down on [`disconnect`](Self::disconnect).
    pub async fn connect(&self, info: &ConnectionInfo) -> Result<()> {
        let (pool, tunnel) = build_pool(info).await?;

        {
            let mut guard = self.pool.write().await;
            *guard = Some(pool);
        }
        {
            let mut guard = self.tunnel.write().await;
            *guard = tunnel;
        }
        Ok(())
    }

    /// Test a connection without storing it. Tunnel (if any) is torn
    /// down when this function returns.
    pub async fn test_connection(info: &ConnectionInfo) -> Result<()> {
        let (pool, _tunnel) = build_test_pool(info).await?;
        match pool {
            Pool::Postgres(p) => {
                sqlx::query("SELECT 1").fetch_one(&p).await?;
                p.close().await;
            }
            Pool::MySql(p) => {
                sqlx::query("SELECT 1").fetch_one(&p).await?;
                p.close().await;
            }
        }
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        let pool = {
            let mut guard = self.pool.write().await;
            guard.take()
        };
        let _tunnel = {
            let mut guard = self.tunnel.write().await;
            guard.take()
        };
        match pool {
            Some(p) => {
                p.close().await;
                Ok(())
            }
            None => Err(anyhow!("No active database connection to disconnect")),
        }
    }

    pub async fn is_connected(&self) -> bool {
        let guard = self.pool.read().await;
        match guard.as_ref() {
            Some(Pool::Postgres(p)) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
            Some(Pool::MySql(p)) => sqlx::query("SELECT 1").fetch_one(p).await.is_ok(),
            None => false,
        }
    }

    // ====================================================================
    // Driver-dispatched API
    // ====================================================================

    pub async fn execute_query_enhanced(&self, sql: &str) -> QueryExecutionResult {
        let guard = self.pool.read().await;
        match guard.as_ref() {
            Some(Pool::Postgres(p)) => pg_backend::query::execute(p, sql).await,
            Some(Pool::MySql(p)) => my_backend::query::execute(p, sql).await,
            None => QueryExecutionResult::Error(ErrorResult {
                message: "Database not connected".to_string(),
                execution_time_ms: 0,
            }),
        }
    }

    pub async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let guard = self.pool.read().await;
        match guard.as_ref() {
            Some(Pool::Postgres(p)) => pg_backend::schema::get_tables(p).await,
            Some(Pool::MySql(p)) => my_backend::schema::get_tables(p).await,
            None => Err(anyhow!("Database not connected")),
        }
    }

    pub async fn get_databases(&self) -> Result<Vec<DatabaseInfo>> {
        let guard = self.pool.read().await;
        match guard.as_ref() {
            Some(Pool::Postgres(p)) => pg_backend::schema::get_databases(p).await,
            Some(Pool::MySql(p)) => my_backend::schema::get_databases(p).await,
            None => Err(anyhow!("Database not connected")),
        }
    }

    pub async fn get_table_columns(
        &self,
        table_name: &str,
        table_schema: &str,
    ) -> Result<QueryExecutionResult> {
        let guard = self.pool.read().await;
        match guard.as_ref() {
            Some(Pool::Postgres(p)) => {
                Ok(pg_backend::schema::get_table_columns(p, table_name, table_schema).await)
            }
            Some(Pool::MySql(p)) => {
                Ok(my_backend::schema::get_table_columns(p, table_name, table_schema).await)
            }
            None => Err(anyhow!("Database not connected")),
        }
    }

    pub async fn get_schema(&self, specific_tables: Option<Vec<String>>) -> Result<DatabaseSchema> {
        let guard = self.pool.read().await;
        match guard.as_ref() {
            Some(Pool::Postgres(p)) => pg_backend::schema::get_schema(p, specific_tables).await,
            Some(Pool::MySql(p)) => my_backend::schema::get_schema(p, specific_tables).await,
            None => Err(anyhow!("Database not connected")),
        }
    }

    /// Streaming row export. Currently Postgres-only; the MySQL export
    /// path falls back to the in-memory `QueryResult` exporter.
    #[allow(dead_code)]
    pub async fn stream_query<'a>(
        &'a self,
        sql: &'a str,
    ) -> Result<BoxStream<'a, Result<PgRow, sqlx::Error>>, String> {
        let guard = self.pool.read().await;
        match guard.as_ref() {
            Some(Pool::Postgres(p)) => {
                let pool = p.clone();
                let stream = sqlx::query(sql).fetch(&pool);
                Ok(stream.boxed())
            }
            Some(Pool::MySql(_)) => {
                Err("Streaming export is not yet implemented for MySQL".to_string())
            }
            None => Err("Database not connected".to_string()),
        }
    }
}

// ============================================================================
// Pool construction
// ============================================================================

/// Build the live pool used by [`DatabaseManager::connect`].
async fn build_pool(info: &ConnectionInfo) -> Result<(Pool, Option<SshTunnel>)> {
    let (host, port, tunnel) = open_tunnel_if_needed(info)?;

    let pool = match info.driver {
        DatabaseDriver::Postgres => {
            let opts = info.to_pg_connect_options_for(&host, port);
            let pool = PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(10))
                .connect_with(opts)
                .await?;
            Pool::Postgres(pool)
        }
        DatabaseDriver::MySql => {
            let opts = info.to_mysql_connect_options_for(&host, port);
            let pool = MySqlPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(10))
                .connect_with(opts)
                .await?;
            Pool::MySql(pool)
        }
    };

    Ok((pool, tunnel))
}

/// Build a one-shot pool used by [`DatabaseManager::test_connection`].
async fn build_test_pool(info: &ConnectionInfo) -> Result<(Pool, Option<SshTunnel>)> {
    let (host, port, tunnel) = open_tunnel_if_needed(info)?;

    let pool = match info.driver {
        DatabaseDriver::Postgres => {
            let opts = info.to_pg_connect_options_for(&host, port);
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(10))
                .connect_with(opts)
                .await?;
            Pool::Postgres(pool)
        }
        DatabaseDriver::MySql => {
            let opts = info.to_mysql_connect_options_for(&host, port);
            let pool = MySqlPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(10))
                .connect_with(opts)
                .await?;
            Pool::MySql(pool)
        }
    };

    Ok((pool, tunnel))
}

/// Returns `(host, port, tunnel)` for the actual TCP endpoint to connect
/// to. When SSH is enabled this is `127.0.0.1:<random>` and `tunnel` is
/// `Some(...)`; otherwise the original host/port.
fn open_tunnel_if_needed(info: &ConnectionInfo) -> Result<(String, u16, Option<SshTunnel>)> {
    match &info.ssh {
        None => Ok((info.hostname.clone(), info.port as u16, None)),
        Some(cfg) => {
            let passphrase = ConnectionsRepository::get_ssh_key_passphrase(&info.id);
            let tunnel = SshTunnel::connect(
                cfg,
                info.hostname.clone(),
                info.port as u16,
                passphrase,
            )?;
            let port = tunnel.local_port();
            Ok(("127.0.0.1".to_string(), port, Some(tunnel)))
        }
    }
}
