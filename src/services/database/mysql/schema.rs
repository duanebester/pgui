//! MySQL schema introspection via `information_schema`.
//!
//! In MySQL, "schema" and "database" are synonyms. We populate
//! `table_schema` with the `TABLE_SCHEMA` column for parity with the
//! Postgres backend; the active database is derived from
//! `DATABASE()` so that listings stay scoped to the connected DB.

use anyhow::Result;
use sqlx::{MySql, MySqlPool, Row};

use crate::services::database::types::{
    ColumnDetail, ConstraintInfo, DatabaseInfo, DatabaseSchema, ForeignKeyInfo, IndexInfo,
    QueryExecutionResult, TableInfo, TableSchema,
};

const SYSTEM_SCHEMAS: &[&str] = &["mysql", "information_schema", "performance_schema", "sys"];

pub async fn get_databases(pool: &MySqlPool) -> Result<Vec<DatabaseInfo>> {
    // SHOW DATABASES is the canonical way; filter out system schemas.
    let rows = sqlx::query("SHOW DATABASES").fetch_all(pool).await?;

    let databases = rows
        .into_iter()
        .filter_map(|row| {
            let name: String = row.try_get(0).ok()?;
            if SYSTEM_SCHEMAS.contains(&name.as_str()) {
                None
            } else {
                Some(DatabaseInfo { datname: name })
            }
        })
        .collect();

    Ok(databases)
}

pub async fn get_tables(pool: &MySqlPool) -> Result<Vec<TableInfo>> {
    let query = r#"
        SELECT
            TABLE_NAME       AS table_name,
            TABLE_SCHEMA     AS table_schema,
            TABLE_TYPE       AS table_type
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY TABLE_SCHEMA, TABLE_NAME
    "#;

    let rows = sqlx::query(query).fetch_all(pool).await?;

    Ok(rows
        .into_iter()
        .map(|row| TableInfo {
            table_name: row.get("table_name"),
            table_schema: row.get("table_schema"),
            table_type: row.get("table_type"),
        })
        .collect())
}

pub async fn get_table_columns(
    pool: &MySqlPool,
    table_name: &str,
    table_schema: &str,
) -> QueryExecutionResult {
    let query_str = r#"
        SELECT
            COLUMN_NAME       AS column_name,
            DATA_TYPE         AS data_type,
            IS_NULLABLE       AS is_nullable,
            COLUMN_DEFAULT    AS column_default,
            ORDINAL_POSITION  AS ordinal_position
        FROM information_schema.COLUMNS
        WHERE TABLE_NAME = ? AND TABLE_SCHEMA = ?
        ORDER BY ORDINAL_POSITION
    "#;

    let query = sqlx::query::<MySql>(query_str)
        .bind(table_name)
        .bind(table_schema);

    super::query::execute_internal(query, pool).await
}

pub async fn get_schema(
    pool: &MySqlPool,
    specific_tables: Option<Vec<String>>,
) -> Result<DatabaseSchema> {
    let table_query = r#"
        SELECT
            TABLE_NAME      AS table_name,
            TABLE_SCHEMA    AS table_schema,
            TABLE_TYPE      AS table_type,
            TABLE_COMMENT   AS description
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY TABLE_SCHEMA, TABLE_NAME
    "#;

    let table_rows = sqlx::query(table_query).fetch_all(pool).await?;
    let mut tables = Vec::new();

    for table_row in table_rows {
        let table_name: String = table_row.get("table_name");
        let table_schema: String = table_row.get("table_schema");
        let table_type: String = table_row.get("table_type");
        let description: Option<String> = table_row
            .try_get::<String, _>("description")
            .ok()
            .filter(|s| !s.is_empty());

        if let Some(ref filter_tables) = specific_tables {
            if !filter_tables.contains(&table_name) {
                continue;
            }
        }

        let columns = fetch_table_columns(&table_name, &table_schema, pool).await?;
        let primary_keys = fetch_primary_keys(&table_name, &table_schema, pool).await?;
        let foreign_keys = fetch_foreign_keys(&table_name, &table_schema, pool).await?;
        let indexes = fetch_indexes(&table_name, &table_schema, pool).await?;
        let constraints = fetch_constraints(&table_name, &table_schema, pool).await?;

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

async fn fetch_table_columns(
    table_name: &str,
    table_schema: &str,
    pool: &MySqlPool,
) -> Result<Vec<ColumnDetail>> {
    let query = r#"
        SELECT
            COLUMN_NAME              AS column_name,
            DATA_TYPE                AS data_type,
            IS_NULLABLE              AS is_nullable,
            COLUMN_DEFAULT           AS column_default,
            ORDINAL_POSITION         AS ordinal_position,
            CHARACTER_MAXIMUM_LENGTH AS character_maximum_length,
            NUMERIC_PRECISION        AS numeric_precision,
            NUMERIC_SCALE            AS numeric_scale,
            COLUMN_COMMENT           AS description
        FROM information_schema.COLUMNS
        WHERE TABLE_NAME = ? AND TABLE_SCHEMA = ?
        ORDER BY ORDINAL_POSITION
    "#;

    let rows = sqlx::query(query)
        .bind(table_name)
        .bind(table_schema)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let is_nullable: String = row.get("is_nullable");
            // information_schema returns these as i64/u64 depending on
            // server config; coerce defensively.
            let character_maximum_length = row
                .try_get::<i64, _>("character_maximum_length")
                .ok()
                .map(|v| v as i32);
            let numeric_precision = row
                .try_get::<i64, _>("numeric_precision")
                .ok()
                .map(|v| v as i32);
            let numeric_scale = row
                .try_get::<i64, _>("numeric_scale")
                .ok()
                .map(|v| v as i32);
            let ordinal_position = row
                .try_get::<i64, _>("ordinal_position")
                .map(|v| v as i32)
                .unwrap_or(0);
            let description = row
                .try_get::<String, _>("description")
                .ok()
                .filter(|s| !s.is_empty());

            ColumnDetail {
                column_name: row.get("column_name"),
                data_type: row.get("data_type"),
                is_nullable: is_nullable == "YES",
                column_default: row.try_get("column_default").ok(),
                ordinal_position,
                character_maximum_length,
                numeric_precision,
                numeric_scale,
                description,
            }
        })
        .collect())
}

async fn fetch_primary_keys(
    table_name: &str,
    table_schema: &str,
    pool: &MySqlPool,
) -> Result<Vec<String>> {
    let query = r#"
        SELECT COLUMN_NAME AS column_name
        FROM information_schema.KEY_COLUMN_USAGE
        WHERE CONSTRAINT_NAME = 'PRIMARY'
            AND TABLE_NAME = ?
            AND TABLE_SCHEMA = ?
        ORDER BY ORDINAL_POSITION
    "#;

    let rows = sqlx::query(query)
        .bind(table_name)
        .bind(table_schema)
        .fetch_all(pool)
        .await?;

    Ok(rows.into_iter().map(|row| row.get("column_name")).collect())
}

async fn fetch_foreign_keys(
    table_name: &str,
    table_schema: &str,
    pool: &MySqlPool,
) -> Result<Vec<ForeignKeyInfo>> {
    let query = r#"
        SELECT
            kcu.CONSTRAINT_NAME        AS constraint_name,
            kcu.COLUMN_NAME            AS column_name,
            kcu.REFERENCED_TABLE_SCHEMA AS foreign_table_schema,
            kcu.REFERENCED_TABLE_NAME  AS foreign_table_name,
            kcu.REFERENCED_COLUMN_NAME AS foreign_column_name
        FROM information_schema.KEY_COLUMN_USAGE kcu
        WHERE kcu.TABLE_NAME = ?
          AND kcu.TABLE_SCHEMA = ?
          AND kcu.REFERENCED_TABLE_NAME IS NOT NULL
        ORDER BY kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION
    "#;

    let rows = sqlx::query(query)
        .bind(table_name)
        .bind(table_schema)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| ForeignKeyInfo {
            constraint_name: row.get("constraint_name"),
            column_name: row.get("column_name"),
            foreign_table_schema: row
                .try_get("foreign_table_schema")
                .unwrap_or_default(),
            foreign_table_name: row.try_get("foreign_table_name").unwrap_or_default(),
            foreign_column_name: row.try_get("foreign_column_name").unwrap_or_default(),
        })
        .collect())
}

async fn fetch_indexes(
    table_name: &str,
    table_schema: &str,
    pool: &MySqlPool,
) -> Result<Vec<IndexInfo>> {
    // Aggregate STATISTICS rows into one entry per index. Use
    // GROUP_CONCAT with ORDER BY SEQ_IN_INDEX so the column list is
    // deterministic and ordered.
    let query = r#"
        SELECT
            INDEX_NAME AS index_name,
            GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX SEPARATOR ',') AS columns,
            MAX(NON_UNIQUE) = 0 AS is_unique,
            (INDEX_NAME = 'PRIMARY') AS is_primary,
            MAX(INDEX_TYPE) AS index_type
        FROM information_schema.STATISTICS
        WHERE TABLE_NAME = ? AND TABLE_SCHEMA = ?
        GROUP BY INDEX_NAME
    "#;

    let rows = sqlx::query(query)
        .bind(table_name)
        .bind(table_schema)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let columns_csv: String = row.try_get("columns").unwrap_or_default();
            let columns = if columns_csv.is_empty() {
                Vec::new()
            } else {
                columns_csv.split(',').map(|s| s.to_string()).collect()
            };
            // is_unique / is_primary come back as i64 (0 or 1).
            let is_unique = row.try_get::<i64, _>("is_unique").unwrap_or(0) != 0;
            let is_primary = row.try_get::<i64, _>("is_primary").unwrap_or(0) != 0;
            IndexInfo {
                index_name: row.get("index_name"),
                columns,
                is_unique,
                is_primary,
                index_type: row.try_get("index_type").unwrap_or_default(),
            }
        })
        .collect())
}

async fn fetch_constraints(
    table_name: &str,
    table_schema: &str,
    pool: &MySqlPool,
) -> Result<Vec<ConstraintInfo>> {
    // UNIQUE constraints + CHECK constraints (CHECK requires MySQL 8.0+).
    // We synthesize the column list from KEY_COLUMN_USAGE for UNIQUEs and
    // pull check_clause from CHECK_CONSTRAINTS for CHECKs.
    let query = r#"
        SELECT
            tc.CONSTRAINT_NAME AS constraint_name,
            tc.CONSTRAINT_TYPE AS constraint_type,
            (
                SELECT GROUP_CONCAT(kcu.COLUMN_NAME ORDER BY kcu.ORDINAL_POSITION SEPARATOR ',')
                FROM information_schema.KEY_COLUMN_USAGE kcu
                WHERE kcu.CONSTRAINT_NAME = tc.CONSTRAINT_NAME
                  AND kcu.TABLE_NAME = tc.TABLE_NAME
                  AND kcu.TABLE_SCHEMA = tc.TABLE_SCHEMA
            ) AS columns,
            (
                SELECT cc.CHECK_CLAUSE
                FROM information_schema.CHECK_CONSTRAINTS cc
                WHERE cc.CONSTRAINT_NAME = tc.CONSTRAINT_NAME
                  AND cc.CONSTRAINT_SCHEMA = tc.TABLE_SCHEMA
                LIMIT 1
            ) AS check_clause
        FROM information_schema.TABLE_CONSTRAINTS tc
        WHERE tc.TABLE_NAME = ?
          AND tc.TABLE_SCHEMA = ?
          AND tc.CONSTRAINT_TYPE IN ('UNIQUE', 'CHECK')
    "#;

    let rows = sqlx::query(query)
        .bind(table_name)
        .bind(table_schema)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let columns_csv: Option<String> = row.try_get("columns").ok();
            let columns = match columns_csv {
                Some(csv) if !csv.is_empty() => csv.split(',').map(|s| s.to_string()).collect(),
                _ => Vec::new(),
            };
            ConstraintInfo {
                constraint_name: row.get("constraint_name"),
                constraint_type: row.get("constraint_type"),
                columns,
                check_clause: row.try_get("check_clause").ok(),
            }
        })
        .collect())
}
