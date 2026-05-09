//! MySQL query execution and row → `QueryResult` conversion.
//!
//! MySQL doesn't expose per-result-column relation OIDs the way Postgres
//! does, so the metadata returned here only fills in `name`, `type_name`
//! and `ordinal`. `table_name` and `is_nullable` are left as `None` for
//! ad-hoc queries; structural lookups should use `schema::get_schema`.

use sqlx::mysql::{MySqlColumn, MySqlPool, MySqlRow};
use sqlx::query::Query;
use sqlx::{Column, Execute as _, Row, TypeInfo, ValueRef};

use crate::services::database::types::{
    ErrorResult, ModifiedResult, QueryExecutionResult, QueryResult, ResultCell,
    ResultColumnMetadata, ResultRow,
};

pub async fn execute(pool: &MySqlPool, sql: &str) -> QueryExecutionResult {
    let sql = sql.trim();
    if sql.is_empty() {
        return QueryExecutionResult::Error(ErrorResult {
            message: "Empty query".to_string(),
            execution_time_ms: 0,
        });
    }

    if is_select_query(sql) {
        execute_select_query(sql, pool).await
    } else {
        execute_modification_query(sql, pool).await
    }
}

async fn execute_modification_query(sql: &str, pool: &MySqlPool) -> QueryExecutionResult {
    let start_time = std::time::Instant::now();
    match sqlx::query(sql).execute(pool).await {
        Ok(result) => QueryExecutionResult::Modified(ModifiedResult {
            rows_affected: result.rows_affected(),
            execution_time_ms: start_time.elapsed().as_millis(),
        }),
        Err(e) => QueryExecutionResult::Error(ErrorResult {
            message: format!("Query failed: {}", e),
            execution_time_ms: start_time.elapsed().as_millis(),
        }),
    }
}

pub(crate) async fn execute_internal(
    query: Query<'_, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    pool: &MySqlPool,
) -> QueryExecutionResult {
    let start_time = std::time::Instant::now();
    let original_query = query.sql().to_string();

    match query.fetch_all(pool).await {
        Ok(rows) => {
            let execution_time = start_time.elapsed().as_millis();

            if rows.is_empty() {
                return QueryExecutionResult::Select(QueryResult {
                    original_query,
                    columns: vec![],
                    rows: vec![],
                    row_count: 0,
                    execution_time_ms: execution_time,
                });
            }

            let columns = build_column_metadata(&rows[0]);
            let result_rows = convert_rows(&rows);

            QueryExecutionResult::Select(QueryResult {
                original_query,
                columns,
                rows: result_rows,
                row_count: rows.len(),
                execution_time_ms: execution_time,
            })
        }
        Err(e) => QueryExecutionResult::Error(ErrorResult {
            message: format!("Query failed: {}", e),
            execution_time_ms: start_time.elapsed().as_millis(),
        }),
    }
}

async fn execute_select_query(sql: &str, pool: &MySqlPool) -> QueryExecutionResult {
    let start_time = std::time::Instant::now();
    let original_query = sql.to_string();

    let limited_sql = if !sql.to_lowercase().contains(" limit ") {
        format!("{} LIMIT {}", sql.trim_end_matches(';'), 1_000)
    } else {
        sql.to_string()
    };

    match sqlx::query(limited_sql.as_ref()).fetch_all(pool).await {
        Ok(rows) => {
            let execution_time = start_time.elapsed().as_millis();

            if rows.is_empty() {
                return QueryExecutionResult::Select(QueryResult {
                    original_query,
                    columns: vec![],
                    rows: vec![],
                    row_count: 0,
                    execution_time_ms: execution_time,
                });
            }

            let columns = build_column_metadata(&rows[0]);
            let result_rows = convert_rows(&rows);

            QueryExecutionResult::Select(QueryResult {
                original_query,
                columns,
                rows: result_rows,
                row_count: rows.len(),
                execution_time_ms: execution_time,
            })
        }
        Err(e) => QueryExecutionResult::Error(ErrorResult {
            message: format!("Query failed: {}", e),
            execution_time_ms: start_time.elapsed().as_millis(),
        }),
    }
}

fn is_select_query(sql: &str) -> bool {
    let lower = sql.to_lowercase();
    let trimmed = lower.trim_start();
    trimmed.starts_with("select")
        || trimmed.starts_with("with")
        || trimmed.starts_with("show")
        || trimmed.starts_with("describe")
        || trimmed.starts_with("desc ")
        || trimmed.starts_with("explain")
}

fn build_column_metadata(first_row: &MySqlRow) -> Vec<ResultColumnMetadata> {
    first_row
        .columns()
        .iter()
        .enumerate()
        .map(|(ordinal, col)| ResultColumnMetadata {
            name: col.name().to_string(),
            type_name: col.type_info().name().to_string(),
            ordinal,
            table_name: None,
            is_nullable: None,
        })
        .collect()
}

fn convert_rows(rows: &[MySqlRow]) -> Vec<ResultRow> {
    rows.iter().map(convert_row).collect()
}

fn convert_row(row: &MySqlRow) -> ResultRow {
    let cells = row
        .columns()
        .iter()
        .enumerate()
        .map(|(i, column)| convert_cell(row, column, i))
        .collect();

    ResultRow { cells }
}

fn build_cell_column_metadata(column: &MySqlColumn, ordinal: usize) -> ResultColumnMetadata {
    ResultColumnMetadata {
        name: column.name().to_string(),
        type_name: column.type_info().name().to_string(),
        ordinal,
        table_name: None,
        is_nullable: None,
    }
}

fn decode_cell_value(row: &MySqlRow, column: &MySqlColumn, index: usize) -> (String, bool) {
    // Try string first — MySQL's text protocol can render most types.
    if let Ok(v) = row.try_get::<String, _>(index) {
        return (v, false);
    }

    match column.type_info().name() {
        "BOOLEAN" | "TINYINT" => row
            .try_get::<i8, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "SMALLINT" => row
            .try_get::<i16, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "MEDIUMINT" | "INT" => row
            .try_get::<i32, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "BIGINT" => row
            .try_get::<i64, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "TINYINT UNSIGNED" => row
            .try_get::<u8, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "SMALLINT UNSIGNED" => row
            .try_get::<u16, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "MEDIUMINT UNSIGNED" | "INT UNSIGNED" => row
            .try_get::<u32, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "BIGINT UNSIGNED" => row
            .try_get::<u64, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "FLOAT" => row
            .try_get::<f32, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "DOUBLE" => row
            .try_get::<f64, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "DECIMAL" => row
            .try_get::<rust_decimal::Decimal, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "DATE" => row
            .try_get::<chrono::NaiveDate, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "TIME" => row
            .try_get::<chrono::NaiveTime, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "DATETIME" => row
            .try_get::<chrono::NaiveDateTime, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "TIMESTAMP" => row
            .try_get::<chrono::DateTime<chrono::Utc>, _>(index)
            .map(|v| (v.to_string(), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BINARY" | "VARBINARY" => row
            .try_get::<Vec<u8>, _>(index)
            .map(|v| (format!("0x{}", hex::encode(&v)), false))
            .unwrap_or_else(|_| ("NULL".to_string(), true)),
        _ => ("NULL".to_string(), true),
    }
}

fn extract_cell_value(row: &MySqlRow, column: &MySqlColumn, index: usize) -> (String, bool) {
    match row.try_get_raw(index) {
        Ok(raw_value) if raw_value.is_null() => ("NULL".to_string(), true),
        Ok(_) => decode_cell_value(row, column, index),
        Err(_) => ("ERROR".to_string(), false),
    }
}

fn convert_cell(row: &MySqlRow, column: &MySqlColumn, index: usize) -> ResultCell {
    let column_metadata = build_cell_column_metadata(column, index);
    let (value, is_null) = extract_cell_value(row, column, index);

    ResultCell {
        value,
        is_null,
        column_metadata,
    }
}
