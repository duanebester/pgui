use anyhow::Result;
use async_std::sync::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::{Column, PgPool, Row, TypeInfo, ValueRef, postgres::PgPoolOptions};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub table_name: String,
    pub table_schema: String,
    pub table_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub execution_time_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub column_name: String,
    pub data_type: String,
    pub is_nullable: String,
    pub column_default: Option<String>,
    pub ordinal_position: i32,
}

#[derive(Debug, Clone)]
pub enum QueryExecutionResult {
    Select(QueryResult),
    Modified {
        rows_affected: u64,
        execution_time_ms: u128,
    },
    Error(String),
}

#[derive(Debug, Clone)]
pub struct DatabaseManager {
    pool: Arc<RwLock<Option<PgPool>>>,
}

impl DatabaseManager {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn connect(&self, database_url: &str) -> Result<()> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        let mut pool_guard = self.pool.write().await;
        *pool_guard = Some(pool);
        Ok(())
    }

    pub async fn disconnect(&self) {
        let mut pool_guard = self.pool.write().await;
        if let Some(pool) = pool_guard.take() {
            pool.close().await;
        }
    }

    #[allow(dead_code)]
    pub async fn is_connected(&self) -> bool {
        let pool_guard = self.pool.read().await;
        pool_guard.is_some()
    }

    pub async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let query = r#"
            SELECT
                table_name,
                table_schema,
                table_type
            FROM information_schema.tables
            WHERE table_schema NOT IN ('information_schema', 'pg_catalog')
            ORDER BY table_schema, table_name
        "#;

        let rows = sqlx::query(query).fetch_all(pool).await?;

        let tables = rows
            .into_iter()
            .map(|row| TableInfo {
                table_name: row.get("table_name"),
                table_schema: row.get("table_schema"),
                table_type: row.get("table_type"),
            })
            .collect();

        Ok(tables)
    }

    pub async fn get_table_columns(
        &self,
        table_name: &str,
        table_schema: &str,
    ) -> Result<QueryResult> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let query = r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                column_default,
                ordinal_position
            FROM information_schema.columns
            WHERE table_name = $1 AND table_schema = $2
            ORDER BY ordinal_position
        "#;

        let rows = sqlx::query(query)
            .bind(table_name)
            .bind(table_schema)
            .fetch_all(pool)
            .await?;

        let columns: Vec<ColumnInfo> = rows
            .into_iter()
            .map(|row| ColumnInfo {
                column_name: row.get("column_name"),
                data_type: row.get("data_type"),
                is_nullable: row.get("is_nullable"),
                column_default: row.get("column_default"),
                ordinal_position: row.get("ordinal_position"),
            })
            .collect();

        // Convert columns to QueryResult format for display
        let column_names = vec![
            "Column Name".to_string(),
            "Data Type".to_string(),
            "Nullable".to_string(),
            "Default".to_string(),
        ];
        let column_rows: Vec<Vec<String>> = columns
            .into_iter()
            .map(|col| {
                vec![
                    col.column_name,
                    col.data_type,
                    col.is_nullable,
                    col.column_default.unwrap_or_else(|| "NULL".to_string()),
                ]
            })
            .collect();

        let query_result = QueryResult {
            columns: column_names,
            rows: column_rows.clone(),
            row_count: column_rows.clone().len(),
            execution_time_ms: 0,
        };

        Ok(query_result)
    }

    pub async fn execute_query(&self, sql: &str) -> QueryExecutionResult {
        let start_time = std::time::Instant::now();

        let pool_guard = match self.pool.read().await {
            pool => pool,
        };

        let pool = match pool_guard.as_ref() {
            Some(pool) => pool,
            None => return QueryExecutionResult::Error("Database not connected".to_string()),
        };

        // Trim whitespace and check if query is empty
        let sql = sql.trim();
        if sql.is_empty() {
            return QueryExecutionResult::Error("Empty query".to_string());
        }

        // Check if this is a SELECT statement (simplified check)
        let is_select = sql.to_lowercase().trim_start().starts_with("select")
            || sql.to_lowercase().trim_start().starts_with("with");

        if is_select {
            match sqlx::query(sql).fetch_all(pool).await {
                Ok(rows) => {
                    let execution_time = start_time.elapsed().as_millis();

                    if rows.is_empty() {
                        return QueryExecutionResult::Select(QueryResult {
                            columns: vec![],
                            rows: vec![],
                            row_count: 0,
                            execution_time_ms: execution_time,
                        });
                    }

                    // Get column names from the first row
                    let columns: Vec<String> = rows[0]
                        .columns()
                        .iter()
                        .map(|col| col.name().to_string())
                        .collect();

                    // Convert rows to string representation
                    let mut result_rows = Vec::new();
                    for row in &rows {
                        let mut string_row = Vec::new();
                        for (i, column) in row.columns().iter().enumerate() {
                            let value = match row.try_get_raw(i) {
                                Ok(raw_value) => {
                                    if raw_value.is_null() {
                                        "NULL".to_string()
                                    } else {
                                        // Try to convert to string representation
                                        match column.type_info().name() {
                                            "BOOL" => row
                                                .try_get::<bool, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "INT2" | "INT4" => row
                                                .try_get::<i32, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "INT8" => row
                                                .try_get::<i64, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "FLOAT4" => row
                                                .try_get::<f32, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "FLOAT8" => row
                                                .try_get::<f64, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "NUMERIC" => {
                                                // Handle DECIMAL/NUMERIC types using rust_decimal
                                                row.try_get::<rust_decimal::Decimal, _>(i)
                                                    .map(|v| v.to_string())
                                                    .unwrap_or_else(|_| {
                                                        // Fallback to string if decimal parsing fails
                                                        row.try_get::<String, _>(i)
                                                            .unwrap_or_else(|_| "NULL".to_string())
                                                    })
                                            }
                                            "MONEY" => {
                                                // PostgreSQL MONEY type - try to get as string first
                                                row.try_get::<String, _>(i)
                                                    .unwrap_or_else(|_| "NULL".to_string())
                                            }
                                            "DATE" | "TIME" | "TIMESTAMP" | "TIMESTAMPTZ"
                                            | "TIMETZ" => {
                                                // Date/time types - get as string
                                                row.try_get::<String, _>(i)
                                                    .unwrap_or_else(|_| "NULL".to_string())
                                            }
                                            "UUID" => {
                                                // UUID type - try to get as string
                                                row.try_get::<String, _>(i)
                                                    .unwrap_or_else(|_| "NULL".to_string())
                                            }
                                            "JSON" | "JSONB" => {
                                                // JSON types - get as string
                                                row.try_get::<String, _>(i)
                                                    .unwrap_or_else(|_| "NULL".to_string())
                                            }
                                            "BYTEA" => {
                                                // Binary data - show as hex or placeholder
                                                match row.try_get::<Vec<u8>, _>(i) {
                                                    Ok(bytes) => format!(
                                                        "\\x{}",
                                                        hex::encode(&bytes[..bytes.len().min(16)])
                                                    ),
                                                    Err(_) => "BINARY".to_string(),
                                                }
                                            }
                                            _ => {
                                                // Default case - try to get as string
                                                row.try_get::<String, _>(i)
                                                    .unwrap_or_else(|_| "NULL".to_string())
                                            }
                                        }
                                    }
                                }
                                Err(_) => "ERROR".to_string(),
                            };
                            string_row.push(value);
                        }
                        result_rows.push(string_row);
                    }

                    QueryExecutionResult::Select(QueryResult {
                        columns,
                        rows: result_rows,
                        row_count: rows.len(),
                        execution_time_ms: execution_time,
                    })
                }
                Err(e) => QueryExecutionResult::Error(format!("Query failed: {}", e)),
            }
        } else {
            // This is an INSERT, UPDATE, DELETE, or other non-SELECT statement
            match sqlx::query(sql).execute(pool).await {
                Ok(result) => {
                    let execution_time = start_time.elapsed().as_millis();
                    QueryExecutionResult::Modified {
                        rows_affected: result.rows_affected(),
                        execution_time_ms: execution_time,
                    }
                }
                Err(e) => QueryExecutionResult::Error(format!("Query failed: {}", e)),
            }
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
