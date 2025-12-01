use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::types::QueryHistoryEntry;

/// Repository for query history operations.
#[derive(Debug, Clone)]
pub struct QueryHistoryRepository {
    pool: SqlitePool,
}

#[allow(dead_code)]
impl QueryHistoryRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record a query execution
    pub async fn record(
        &self,
        connection_id: &Uuid,
        sql: &str,
        execution_time_ms: i64,
        rows_affected: Option<i64>,
        success: bool,
        error_message: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO query_history
                (id, connection_id, sql, execution_time_ms, rows_affected, success, error_message, executed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(connection_id.to_string())
        .bind(sql)
        .bind(execution_time_ms)
        .bind(rows_affected)
        .bind(success)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load history for a specific connection (most recent first)
    pub async fn load_for_connection(
        &self,
        connection_id: &Uuid,
        limit: u32,
    ) -> Result<Vec<QueryHistoryEntry>> {
        let rows = sqlx::query_as::<_, (String, String, String, i64, Option<i64>, bool, Option<String>, String)>(
            r#"
            SELECT id, connection_id, sql, execution_time_ms, rows_affected, success, error_message, executed_at
            FROM query_history
            WHERE connection_id = ?
            ORDER BY executed_at DESC
            LIMIT ?
            "#,
        )
        .bind(connection_id.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(
                |(id, conn_id, sql, exec_time, rows, success, err, executed_at)| {
                    Ok(QueryHistoryEntry {
                        id: Uuid::parse_str(&id).context("Invalid UUID")?,
                        connection_id: Uuid::parse_str(&conn_id)
                            .context("Invalid connection UUID")?,
                        sql,
                        execution_time_ms: exec_time,
                        rows_affected: rows,
                        success,
                        error_message: err,
                        executed_at: NaiveDateTime::parse_from_str(
                            &executed_at,
                            "%Y-%m-%d %H:%M:%S",
                        )
                        .map(|dt| dt.and_utc())
                        .unwrap_or_else(|_| Utc::now()),
                    })
                },
            )
            .collect()
    }

    /// Clear history for a connection
    pub async fn clear_for_connection(&self, connection_id: &Uuid) -> Result<()> {
        sqlx::query("DELETE FROM query_history WHERE connection_id = ?")
            .bind(connection_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Prune old entries, keeping only the last N per connection
    pub async fn prune(&self, keep_per_connection: u32) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM query_history
            WHERE id NOT IN (
                SELECT id FROM (
                    SELECT id, ROW_NUMBER() OVER (
                        PARTITION BY connection_id
                        ORDER BY executed_at DESC
                    ) as rn
                    FROM query_history
                ) ranked
                WHERE rn <= ?
            )
            "#,
        )
        .bind(keep_per_connection)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}
