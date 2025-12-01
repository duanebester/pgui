use sqlx::postgres::types::Oid;
use sqlx::postgres::{PgColumn, PgRow};
use sqlx::query::Query;
use sqlx::{Column, PgPool, Row, TypeInfo, ValueRef};
use std::collections::{HashMap, HashSet};

use crate::services::database::types::{ErrorResult, ModifiedResult};

use super::manager::DatabaseManager;
use super::types::{
    QueryExecutionResult, QueryResult, ResultCell, ResultColumnMetadata, ResultRow, TableMetadata,
};

impl DatabaseManager {
    pub async fn execute_query_enhanced(&self, sql: &str) -> QueryExecutionResult {
        let pool_guard = self.pool.read().await;

        let pool = match pool_guard.as_ref() {
            Some(pool) => pool,
            None => {
                return QueryExecutionResult::Error(ErrorResult {
                    message: "Database not connected".to_string(),
                    execution_time_ms: 0,
                });
            }
        };

        let sql = sql.trim();
        if sql.is_empty() {
            return QueryExecutionResult::Error(ErrorResult {
                message: "Empty query".to_string(),
                execution_time_ms: 0,
            });
        }

        if is_select_query(sql) {
            self.execute_select_query(sql, pool).await
        } else {
            self.execute_modification_query(sql, pool).await
        }
    }

    async fn execute_select_query(&self, sql: &str, pool: &PgPool) -> QueryExecutionResult {
        let q = sqlx::query(sql);
        self.execute_base_query(q, pool).await
    }

    async fn execute_modification_query(&self, sql: &str, pool: &PgPool) -> QueryExecutionResult {
        let start_time = std::time::Instant::now();
        match sqlx::query(sql).execute(pool).await {
            Ok(result) => {
                let execution_time_ms = start_time.elapsed().as_millis();
                QueryExecutionResult::Modified(ModifiedResult {
                    rows_affected: result.rows_affected(),
                    execution_time_ms,
                })
            }
            Err(e) => {
                let execution_time_ms = start_time.elapsed().as_millis();
                QueryExecutionResult::Error(ErrorResult {
                    message: format!("Query failed: {}", e),
                    execution_time_ms,
                })
            }
        }
    }

    pub(crate) async fn execute_base_query(
        &self,
        query: Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>,
        pool: &PgPool,
    ) -> QueryExecutionResult {
        let start_time = std::time::Instant::now();
        match query.fetch_all(pool).await {
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

                let metadata = fetch_table_metadata(&rows, pool).await;
                let columns = build_column_metadata(&rows[0], &metadata);
                let result_rows = convert_rows(&rows, &metadata);

                QueryExecutionResult::Select(QueryResult {
                    columns,
                    rows: result_rows,
                    row_count: rows.len(),
                    execution_time_ms: execution_time,
                })
            }
            Err(e) => {
                let execution_time_ms = start_time.elapsed().as_millis();
                QueryExecutionResult::Error(ErrorResult {
                    message: format!("Query failed: {}", e),
                    execution_time_ms,
                })
            }
        }
    }
}

// ============================================================================
// Free functions for query processing
// ============================================================================

fn is_select_query(sql: &str) -> bool {
    let lower = sql.to_lowercase();
    let trimmed = lower.trim_start();
    trimmed.starts_with("select") || trimmed.starts_with("with")
}

async fn fetch_table_metadata(rows: &[PgRow], pool: &PgPool) -> TableMetadata {
    let mut relation_oids = HashSet::new();

    for col in rows[0].columns() {
        if let Some(oid) = col.relation_id() {
            relation_oids.insert(oid);
        }
    }

    let mut oid_to_table_name: HashMap<Oid, String> = HashMap::new();
    let mut column_nullable_map: HashMap<(Oid, String), bool> = HashMap::new();

    for oid in relation_oids {
        if let Some(table_name) = fetch_table_name(oid, pool).await {
            oid_to_table_name.insert(oid, table_name);
        }

        if let Ok(nullable_info) = fetch_nullable_info(oid, pool).await {
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

async fn fetch_nullable_info(oid: Oid, pool: &PgPool) -> Result<Vec<(String, bool)>, sqlx::Error> {
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

fn build_column_metadata(first_row: &PgRow, metadata: &TableMetadata) -> Vec<ResultColumnMetadata> {
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
    rows.iter().map(|row| convert_row(row, metadata)).collect()
}

fn convert_row(row: &PgRow, metadata: &TableMetadata) -> ResultRow {
    let cells = row
        .columns()
        .iter()
        .enumerate()
        .map(|(i, column)| convert_cell(row, column, i, metadata))
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
        Ok(_) => decode_cell_value(row, column, index),
        Err(_) => ("ERROR".to_string(), false),
    }
}

fn convert_cell(
    row: &PgRow,
    column: &PgColumn,
    index: usize,
    metadata: &TableMetadata,
) -> ResultCell {
    let column_metadata = build_cell_column_metadata(column, index, metadata);
    let (value, is_null) = extract_cell_value(row, column, index);

    ResultCell {
        value,
        is_null,
        column_metadata,
    }
}
