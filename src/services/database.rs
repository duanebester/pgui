use anyhow::Result;
use async_std::sync::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::{
    Column, PgPool, Row, TypeInfo, ValueRef,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{sync::Arc, time::Duration};

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

// New structs for comprehensive schema information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub table_name: String,
    pub table_schema: String,
    pub table_type: String,
    pub columns: Vec<ColumnDetail>,
    pub primary_keys: Vec<String>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub indexes: Vec<IndexInfo>,
    pub constraints: Vec<ConstraintInfo>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDetail {
    pub column_name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub column_default: Option<String>,
    pub ordinal_position: i32,
    pub character_maximum_length: Option<i32>,
    pub numeric_precision: Option<i32>,
    pub numeric_scale: Option<i32>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyInfo {
    pub constraint_name: String,
    pub column_name: String,
    pub foreign_table_schema: String,
    pub foreign_table_name: String,
    pub foreign_column_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub index_name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_primary: bool,
    pub index_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintInfo {
    pub constraint_name: String,
    pub constraint_type: String,
    pub columns: Vec<String>,
    pub check_clause: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    pub tables: Vec<TableSchema>,
    pub total_tables: usize,
}

#[derive(Debug, Clone)]
pub struct DatabaseManager {
    pool: Arc<RwLock<Option<PgPool>>>,
}

#[allow(dead_code)]
impl DatabaseManager {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(RwLock::new(None)),
        }
    }

    #[deprecated(since = "0.1.2", note = "Use connect_with_options() instead")]
    pub async fn connect(&self, database_url: &str) -> Result<()> {
        let pool_opts = PgPoolOptions::new()
            .max_connections(5) // Increased from 1
            .acquire_timeout(Duration::from_secs(5)); // Increased timeout

        let pool = pool_opts.connect(database_url).await?;

        let mut pool_guard = self.pool.write().await;
        *pool_guard = Some(pool);
        Ok(())
    }

    pub async fn connect_with_options(&self, options: PgConnectOptions) -> Result<()> {
        let pool_opts = PgPoolOptions::new()
            .max_connections(5) // Better than 1 for UI responsiveness
            .acquire_timeout(Duration::from_secs(5));

        let pool = pool_opts.connect_with(options).await?;

        let mut pool_guard = self.pool.write().await;
        *pool_guard = Some(pool);
        Ok(())
    }

    pub async fn test_connection_options(options: PgConnectOptions) -> Result<()> {
        // Create a temporary connection just for testing
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await?;

        // Test with a simple query
        sqlx::query("SELECT 1").fetch_one(&pool).await?;

        // Close the test connection
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
            // Test connection with a simple query
            sqlx::query("SELECT 1").fetch_one(pool).await.is_ok()
        } else {
            false
        }
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
        let start_time = std::time::Instant::now();

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

        let execution_time_ms = start_time.elapsed().as_millis();

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

        let row_count = column_rows.len(); // Store length before moving

        let query_result = QueryResult {
            columns: column_names,
            rows: column_rows,
            row_count,
            execution_time_ms,
        };

        Ok(query_result)
    }

    /// Retrieves comprehensive schema information for all tables or specific tables
    pub async fn get_schema(&self, specific_tables: Option<Vec<String>>) -> Result<DatabaseSchema> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        // Get all tables
        let table_query = r#"
            SELECT
                t.table_name,
                t.table_schema,
                t.table_type,
                obj_description((t.table_schema || '.' || t.table_name)::regclass, 'pg_class') as description
            FROM information_schema.tables t
            WHERE t.table_schema NOT IN ('information_schema', 'pg_catalog')
            ORDER BY t.table_schema, t.table_name
        "#;

        let table_rows = sqlx::query(table_query).fetch_all(pool).await?;
        let mut tables = Vec::new();

        for table_row in table_rows {
            let table_name: String = table_row.get("table_name");
            let table_schema: String = table_row.get("table_schema");
            let table_type: String = table_row.get("table_type");
            let description: Option<String> = table_row.get("description");

            // Filter by specific tables if requested
            if let Some(ref filter_tables) = specific_tables {
                if !filter_tables.contains(&table_name) {
                    continue;
                }
            }

            // Get columns
            let column_query = r#"
                SELECT
                    c.column_name,
                    c.data_type,
                    c.is_nullable,
                    c.column_default,
                    c.ordinal_position,
                    c.character_maximum_length,
                    c.numeric_precision,
                    c.numeric_scale,
                    col_description((c.table_schema || '.' || c.table_name)::regclass, c.ordinal_position) as description
                FROM information_schema.columns c
                WHERE c.table_name = $1 AND c.table_schema = $2
                ORDER BY c.ordinal_position
            "#;

            let column_rows = sqlx::query(column_query)
                .bind(&table_name)
                .bind(&table_schema)
                .fetch_all(pool)
                .await?;

            let columns: Vec<ColumnDetail> = column_rows
                .into_iter()
                .map(|row| {
                    let is_nullable: String = row.get("is_nullable");
                    ColumnDetail {
                        column_name: row.get("column_name"),
                        data_type: row.get("data_type"),
                        is_nullable: is_nullable == "YES",
                        column_default: row.get("column_default"),
                        ordinal_position: row.get("ordinal_position"),
                        character_maximum_length: row.get("character_maximum_length"),
                        numeric_precision: row.get("numeric_precision"),
                        numeric_scale: row.get("numeric_scale"),
                        description: row.get("description"),
                    }
                })
                .collect();

            // Get primary keys
            let pk_query = r#"
                SELECT kcu.column_name
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                    ON tc.constraint_name = kcu.constraint_name
                    AND tc.table_schema = kcu.table_schema
                WHERE tc.constraint_type = 'PRIMARY KEY'
                    AND tc.table_name = $1
                    AND tc.table_schema = $2
                ORDER BY kcu.ordinal_position
            "#;

            let pk_rows = sqlx::query(pk_query)
                .bind(&table_name)
                .bind(&table_schema)
                .fetch_all(pool)
                .await?;

            let primary_keys: Vec<String> = pk_rows
                .into_iter()
                .map(|row| row.get("column_name"))
                .collect();

            // Get foreign keys
            let fk_query = r#"
                SELECT
                    tc.constraint_name,
                    kcu.column_name,
                    ccu.table_schema AS foreign_table_schema,
                    ccu.table_name AS foreign_table_name,
                    ccu.column_name AS foreign_column_name
                FROM information_schema.table_constraints AS tc
                JOIN information_schema.key_column_usage AS kcu
                    ON tc.constraint_name = kcu.constraint_name
                    AND tc.table_schema = kcu.table_schema
                JOIN information_schema.constraint_column_usage AS ccu
                    ON ccu.constraint_name = tc.constraint_name
                    AND ccu.table_schema = tc.table_schema
                WHERE tc.constraint_type = 'FOREIGN KEY'
                    AND tc.table_name = $1
                    AND tc.table_schema = $2
            "#;

            let fk_rows = sqlx::query(fk_query)
                .bind(&table_name)
                .bind(&table_schema)
                .fetch_all(pool)
                .await?;

            let foreign_keys: Vec<ForeignKeyInfo> = fk_rows
                .into_iter()
                .map(|row| ForeignKeyInfo {
                    constraint_name: row.get("constraint_name"),
                    column_name: row.get("column_name"),
                    foreign_table_schema: row.get("foreign_table_schema"),
                    foreign_table_name: row.get("foreign_table_name"),
                    foreign_column_name: row.get("foreign_column_name"),
                })
                .collect();

            // Get indexes
            let index_query = r#"
                SELECT
                    i.relname as index_name,
                    array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum)) as columns,
                    ix.indisunique as is_unique,
                    ix.indisprimary as is_primary,
                    am.amname as index_type
                FROM pg_class t
                JOIN pg_index ix ON t.oid = ix.indrelid
                JOIN pg_class i ON i.oid = ix.indexrelid
                JOIN pg_am am ON i.relam = am.oid
                JOIN pg_namespace n ON t.relnamespace = n.oid
                LEFT JOIN unnest(ix.indkey) WITH ORDINALITY AS u(attnum, ord) ON true
                LEFT JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = u.attnum
                WHERE t.relname = $1
                    AND n.nspname = $2
                    AND a.attname IS NOT NULL
                GROUP BY i.relname, ix.indisunique, ix.indisprimary, am.amname
            "#;

            let index_rows = sqlx::query(index_query)
                .bind(&table_name)
                .bind(&table_schema)
                .fetch_all(pool)
                .await?;

            let indexes: Vec<IndexInfo> = index_rows
                .into_iter()
                .map(|row| IndexInfo {
                    index_name: row.get("index_name"),
                    columns: row.get("columns"),
                    is_unique: row.get("is_unique"),
                    is_primary: row.get("is_primary"),
                    index_type: row.get("index_type"),
                })
                .collect();

            // Get constraints (check, unique, etc.)
            let constraint_query = r#"
                SELECT
                    tc.constraint_name,
                    tc.constraint_type,
                    COALESCE(array_agg(kcu.column_name::TEXT) FILTER (WHERE kcu.column_name IS NOT NULL), ARRAY[]::TEXT[]) as columns,
                    cc.check_clause
                FROM information_schema.table_constraints tc
                LEFT JOIN information_schema.key_column_usage kcu
                    ON tc.constraint_name = kcu.constraint_name
                    AND tc.table_schema = kcu.table_schema
                LEFT JOIN information_schema.check_constraints cc
                    ON tc.constraint_name = cc.constraint_name
                    AND tc.constraint_schema = cc.constraint_schema
                WHERE tc.table_name = $1
                    AND tc.table_schema = $2
                    AND tc.constraint_type IN ('UNIQUE', 'CHECK')
                GROUP BY tc.constraint_name, tc.constraint_type, cc.check_clause
            "#;

            let constraint_rows = sqlx::query(constraint_query)
                .bind(&table_name)
                .bind(&table_schema)
                .fetch_all(pool)
                .await?;

            let constraints: Vec<ConstraintInfo> = constraint_rows
                .into_iter()
                .map(|row| ConstraintInfo {
                    constraint_name: row.get("constraint_name"),
                    constraint_type: row.get("constraint_type"),
                    columns: row.get("columns"),
                    check_clause: row.get("check_clause"),
                })
                .collect();

            tables.push(TableSchema {
                table_name,
                table_schema,
                table_type,
                columns,
                primary_keys,
                foreign_keys,
                indexes,
                constraints,
                description,
            });
        }

        let total_tables = tables.len();

        Ok(DatabaseSchema {
            tables,
            total_tables,
        })
    }

    /// Generates a human-readable schema description for LLM consumption
    pub fn format_schema_for_llm(&self, schema: &DatabaseSchema) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "# Database Schema ({} tables)\n\n",
            schema.total_tables
        ));

        for table in &schema.tables {
            output.push_str(&format!(
                "## Table: {}.{}\n",
                table.table_schema, table.table_name
            ));
            output.push_str(&format!("Type: {}\n", table.table_type));

            if let Some(ref desc) = table.description {
                output.push_str(&format!("Description: {}\n", desc));
            }
            output.push('\n');

            // Columns
            output.push_str("### Columns:\n");
            for col in &table.columns {
                let nullable = if col.is_nullable { "NULL" } else { "NOT NULL" };
                let mut col_line =
                    format!("- **{}**: {} {}", col.column_name, col.data_type, nullable);

                if let Some(ref default) = col.column_default {
                    col_line.push_str(&format!(" DEFAULT {}", default));
                }

                if let Some(len) = col.character_maximum_length {
                    col_line.push_str(&format!(" (max length: {})", len));
                }

                if let Some(prec) = col.numeric_precision {
                    if let Some(scale) = col.numeric_scale {
                        col_line.push_str(&format!(" (precision: {}, scale: {})", prec, scale));
                    }
                }

                if let Some(ref desc) = col.description {
                    col_line.push_str(&format!(" - {}", desc));
                }

                output.push_str(&col_line);
                output.push('\n');
            }
            output.push('\n');

            // Primary Keys
            if !table.primary_keys.is_empty() {
                output.push_str(&format!(
                    "### Primary Key: {}\n\n",
                    table.primary_keys.join(", ")
                ));
            }

            // Foreign Keys
            if !table.foreign_keys.is_empty() {
                output.push_str("### Foreign Keys:\n");
                for fk in &table.foreign_keys {
                    output.push_str(&format!(
                        "- **{}** → {}.{}.{}\n",
                        fk.column_name,
                        fk.foreign_table_schema,
                        fk.foreign_table_name,
                        fk.foreign_column_name
                    ));
                }
                output.push('\n');
            }

            // Indexes
            if !table.indexes.is_empty() {
                output.push_str("### Indexes:\n");
                for idx in &table.indexes {
                    let unique = if idx.is_unique { "UNIQUE" } else { "" };
                    let primary = if idx.is_primary { "PRIMARY" } else { "" };
                    let flags = [unique, primary]
                        .iter()
                        .filter(|s| !s.is_empty())
                        .map(|s| *s)
                        .collect::<Vec<_>>()
                        .join(", ");
                    let flags_str = if !flags.is_empty() {
                        format!(" [{}]", flags)
                    } else {
                        String::new()
                    };

                    output.push_str(&format!(
                        "- **{}** ({}):{} {}\n",
                        idx.index_name,
                        idx.index_type,
                        flags_str,
                        idx.columns.join(", ")
                    ));
                }
                output.push('\n');
            }

            // Constraints
            if !table.constraints.is_empty() {
                output.push_str("### Constraints:\n");
                for constraint in &table.constraints {
                    output.push_str(&format!(
                        "- **{}** ({}): {}",
                        constraint.constraint_name,
                        constraint.constraint_type,
                        constraint.columns.join(", ")
                    ));

                    if let Some(ref check) = constraint.check_clause {
                        output.push_str(&format!(" WHERE {}", check));
                    }
                    output.push('\n');
                }
                output.push('\n');
            }

            output.push_str("---\n\n");
        }

        output
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
