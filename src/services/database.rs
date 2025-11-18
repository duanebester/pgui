use anyhow::Result;
use async_std::sync::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::{
    Column, PgPool, Row, TypeInfo, ValueRef,
    postgres::{PgColumn, PgConnectOptions, PgPoolOptions, PgRow, types::Oid},
    query::Query,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub table_name: String,
    pub table_schema: String,
    pub table_type: String,
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

// ============================================================================
// Enhanced Query Result Structures with Full Metadata
// ============================================================================

/// Metadata about a column from a query result
/// This is database-agnostic but captures PostgreSQL-specific info when available
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultColumnMetadata {
    pub name: String,
    pub type_name: String,
    pub ordinal: usize,
    /// The source table name (if available from query metadata)
    pub table_name: Option<String>,
    /// Whether the column allows NULL values
    pub is_nullable: Option<bool>,
}

/// A cell value with its metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultCell {
    /// String representation of the value
    pub value: String,
    /// Whether the value is NULL
    pub is_null: bool,
    /// Column metadata for this cell
    pub column_metadata: ResultColumnMetadata,
}

/// A row with full metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultRow {
    pub cells: Vec<ResultCell>,
}

/// Enhanced query result with full metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedQueryResult {
    pub columns: Vec<ResultColumnMetadata>,
    pub rows: Vec<ResultRow>,
    pub row_count: usize,
    pub execution_time_ms: u128,
}

/// Result of an enhanced query execution
#[derive(Debug, Clone)]
pub enum EnhancedQueryExecutionResult {
    Select(EnhancedQueryResult),
    Modified {
        rows_affected: u64,
        execution_time_ms: u128,
    },
    Error(String),
}

struct TableMetadata {
    oid_to_table_name: HashMap<Oid, String>,
    column_nullable_map: HashMap<(Oid, String), bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub datname: String,
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

    pub async fn connect_with_options(&self, options: PgConnectOptions) -> Result<()> {
        let pool_opts = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5));

        let pool = pool_opts.connect_with(options).await?;

        let mut pool_guard = self.pool.write().await;
        *pool_guard = Some(pool);
        Ok(())
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub async fn get_databases(&self) -> Result<Vec<DatabaseInfo>> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let query = r#"
            SELECT datname
            FROM pg_database
            WHERE datistemplate = false
            -- AND datname != 'postgres'
            ORDER BY datname
        "#;

        let rows = sqlx::query(query).fetch_all(pool).await?;

        let databases = rows
            .into_iter()
            .map(|row| DatabaseInfo {
                datname: row.get("datname"),
            })
            .collect();

        Ok(databases)
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
    ) -> Result<EnhancedQueryExecutionResult> {
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

        let query = sqlx::query(query).bind(table_name).bind(table_schema);
        let result = self.execute_base_query(query, pool).await;
        Ok(result)
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

    #[allow(dead_code)]
    pub async fn test_connection(&self) -> Result<bool> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let _: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pool).await?;
        Ok(true)
    }

    fn is_select_query(sql: &str) -> bool {
        let lower = sql.to_lowercase();
        let trimmed = lower.trim_start();
        trimmed.starts_with("select") || trimmed.starts_with("with")
    }

    async fn execute_modification_query(
        &self,
        sql: &str,
        pool: &PgPool,
    ) -> EnhancedQueryExecutionResult {
        let start_time = std::time::Instant::now();
        match sqlx::query(sql).execute(pool).await {
            Ok(result) => {
                let execution_time = start_time.elapsed().as_millis();
                EnhancedQueryExecutionResult::Modified {
                    rows_affected: result.rows_affected(),
                    execution_time_ms: execution_time,
                }
            }
            Err(e) => EnhancedQueryExecutionResult::Error(format!("Query failed: {}", e)),
        }
    }

    async fn fetch_table_name(oid: Oid, pool: &PgPool) -> Option<String> {
        let query = r#"
                SELECT n.nspname || '.' || c.relname as full_name
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE c.oid = $1
            "#;

        sqlx::query(query)
            .bind(&oid)
            .fetch_one(pool)
            .await
            .ok()?
            .try_get::<String, _>(0)
            .ok()
    }

    async fn fetch_nullable_info(
        oid: Oid,
        pool: &PgPool,
    ) -> Result<Vec<(String, bool)>, sqlx::Error> {
        let query = r#"
                SELECT attname, NOT attnotnull as is_nullable
                FROM pg_attribute
                WHERE attrelid = $1
                AND attnum > 0
                AND NOT attisdropped
            "#;

        let rows = sqlx::query(query).bind(&oid).fetch_all(pool).await?;

        Ok(rows
            .iter()
            .filter_map(
                |row| match (row.try_get::<String, _>(0), row.try_get::<bool, _>(1)) {
                    (Ok(col_name), Ok(is_nullable)) => Some((col_name, is_nullable)),
                    _ => None,
                },
            )
            .collect())
    }

    async fn fetch_table_metadata(&self, rows: &[PgRow], pool: &PgPool) -> TableMetadata {
        let mut relation_oids = HashSet::new();

        for col in rows[0].columns() {
            if let Some(oid) = col.relation_id() {
                relation_oids.insert(oid);
            }
        }

        let mut oid_to_table_name: HashMap<Oid, String> = HashMap::new();
        let mut column_nullable_map: HashMap<(Oid, String), bool> = HashMap::new();

        for oid in relation_oids {
            if let Some(table_name) = Self::fetch_table_name(oid, pool).await {
                oid_to_table_name.insert(oid, table_name);
            }

            if let Ok(nullable_info) = Self::fetch_nullable_info(oid, pool).await {
                for (col_name, is_nullable) in nullable_info {
                    column_nullable_map.insert((oid, col_name), is_nullable);
                }
            }
        }

        TableMetadata {
            oid_to_table_name,
            column_nullable_map,
        }
    }

    fn build_column_metadata(
        first_row: &PgRow,
        metadata: &TableMetadata,
    ) -> Vec<ResultColumnMetadata> {
        first_row
            .columns()
            .iter()
            .enumerate()
            .map(|(ordinal, col)| {
                let table_name = col
                    .relation_id()
                    .and_then(|oid| metadata.oid_to_table_name.get(&oid).cloned());

                let is_nullable = col.relation_id().and_then(|oid| {
                    metadata
                        .column_nullable_map
                        .get(&(oid, col.name().to_string()))
                        .copied()
                });

                ResultColumnMetadata {
                    name: col.name().to_string(),
                    type_name: col.type_info().name().to_string(),
                    ordinal,
                    table_name,
                    is_nullable,
                }
            })
            .collect()
    }

    fn convert_rows(rows: &[PgRow], metadata: &TableMetadata) -> Vec<ResultRow> {
        rows.iter()
            .map(|row| Self::convert_row(row, metadata))
            .collect()
    }

    fn convert_row(row: &PgRow, metadata: &TableMetadata) -> ResultRow {
        let cells = row
            .columns()
            .iter()
            .enumerate()
            .map(|(i, column)| Self::convert_cell(row, column, i, metadata))
            .collect();

        ResultRow { cells }
    }

    fn build_cell_column_metadata(
        column: &PgColumn,
        ordinal: usize,
        metadata: &TableMetadata,
    ) -> ResultColumnMetadata {
        let table_name = column
            .relation_id()
            .and_then(|oid| metadata.oid_to_table_name.get(&oid).cloned());

        let is_nullable = column.relation_id().and_then(|oid| {
            metadata
                .column_nullable_map
                .get(&(oid, column.name().to_string()))
                .copied()
        });

        ResultColumnMetadata {
            name: column.name().to_string(),
            type_name: column.type_info().name().to_string(),
            ordinal,
            table_name,
            is_nullable,
        }
    }

    fn decode_cell_value(row: &PgRow, column: &PgColumn, index: usize) -> (String, bool) {
        // Try to decode as String first - Postgres can convert most types to text
        if let Ok(v) = row.try_get::<String, _>(index) {
            return (v, false);
        }

        // If string decoding fails, try type-specific decoding
        match column.type_info().name() {
            "BOOL" => row
                .try_get::<bool, _>(index)
                .map(|v| (v.to_string(), false))
                .unwrap_or_else(|_| ("NULL".to_string(), true)),
            "INT2" | "INT4" => row
                .try_get::<i32, _>(index)
                .map(|v| (v.to_string(), false))
                .unwrap_or_else(|_| ("NULL".to_string(), true)),
            "INT8" => row
                .try_get::<i64, _>(index)
                .map(|v| (v.to_string(), false))
                .unwrap_or_else(|_| ("NULL".to_string(), true)),
            "FLOAT4" => row
                .try_get::<f32, _>(index)
                .map(|v| (v.to_string(), false))
                .unwrap_or_else(|_| ("NULL".to_string(), true)),
            "FLOAT8" => row
                .try_get::<f64, _>(index)
                .map(|v| (v.to_string(), false))
                .unwrap_or_else(|_| ("NULL".to_string(), true)),
            "NUMERIC" => row
                .try_get::<rust_decimal::Decimal, _>(index)
                .map(|v| (v.to_string(), false))
                .unwrap_or_else(|_| ("NULL".to_string(), true)),
            _ => ("NULL".to_string(), true),
        }
    }

    fn extract_cell_value(row: &PgRow, column: &PgColumn, index: usize) -> (String, bool) {
        match row.try_get_raw(index) {
            Ok(raw_value) if raw_value.is_null() => ("NULL".to_string(), true),
            Ok(_) => Self::decode_cell_value(row, column, index),
            Err(_) => ("ERROR".to_string(), false),
        }
    }

    fn convert_cell(
        row: &PgRow,
        column: &PgColumn,
        index: usize,
        metadata: &TableMetadata,
    ) -> ResultCell {
        let column_metadata = Self::build_cell_column_metadata(column, index, metadata);
        let (value, is_null) = Self::extract_cell_value(row, column, index);

        ResultCell {
            value,
            is_null,
            column_metadata,
        }
    }

    async fn execute_select_query(&self, sql: &str, pool: &PgPool) -> EnhancedQueryExecutionResult {
        let q = sqlx::query(sql);
        self.execute_base_query(q, pool).await
    }

    async fn execute_base_query(
        &self,
        query: Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>,
        pool: &PgPool,
    ) -> EnhancedQueryExecutionResult {
        let start_time = std::time::Instant::now();
        match query.fetch_all(pool).await {
            Ok(rows) => {
                let execution_time = start_time.elapsed().as_millis();

                if rows.is_empty() {
                    return EnhancedQueryExecutionResult::Select(EnhancedQueryResult {
                        columns: vec![],
                        rows: vec![],
                        row_count: 0,
                        execution_time_ms: execution_time,
                    });
                }

                let metadata = self.fetch_table_metadata(&rows, pool).await;
                let columns = Self::build_column_metadata(&rows[0], &metadata);
                let result_rows = Self::convert_rows(&rows, &metadata);

                EnhancedQueryExecutionResult::Select(EnhancedQueryResult {
                    columns,
                    rows: result_rows,
                    row_count: rows.len(),
                    execution_time_ms: execution_time,
                })
            }
            Err(e) => EnhancedQueryExecutionResult::Error(format!("Query failed: {}", e)),
        }
    }

    pub async fn execute_query_enhanced(&self, sql: &str) -> EnhancedQueryExecutionResult {
        let pool_guard = match self.pool.read().await {
            pool => pool,
        };

        let pool = match pool_guard.as_ref() {
            Some(pool) => pool,
            None => {
                return EnhancedQueryExecutionResult::Error("Database not connected".to_string());
            }
        };

        let sql = sql.trim();
        if sql.is_empty() {
            return EnhancedQueryExecutionResult::Error("Empty query".to_string());
        }

        if Self::is_select_query(sql) {
            self.execute_select_query(sql, pool).await
        } else {
            self.execute_modification_query(sql, pool).await
        }
    }
}
