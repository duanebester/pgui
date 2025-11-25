use anyhow::Result;
use sqlx::Row;

use super::manager::DatabaseManager;
use super::types::{
    ColumnDetail, ConstraintInfo, DatabaseInfo, DatabaseSchema, EnhancedQueryExecutionResult,
    ForeignKeyInfo, IndexInfo, TableInfo, TableSchema,
};

impl DatabaseManager {
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

            if let Some(ref filter_tables) = specific_tables {
                if !filter_tables.contains(&table_name) {
                    continue;
                }
            }

            // Get columns
            let columns = self
                .fetch_table_columns(&table_name, &table_schema, pool)
                .await?;

            // Get primary keys
            let primary_keys = self
                .fetch_primary_keys(&table_name, &table_schema, pool)
                .await?;

            // Get foreign keys
            let foreign_keys = self
                .fetch_foreign_keys(&table_name, &table_schema, pool)
                .await?;

            // Get indexes
            let indexes = self.fetch_indexes(&table_name, &table_schema, pool).await?;

            // Get constraints
            let constraints = self
                .fetch_constraints(&table_name, &table_schema, pool)
                .await?;

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
        &self,
        table_name: &str,
        table_schema: &str,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<ColumnDetail>> {
        let query = r#"
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

        let rows = sqlx::query(query)
            .bind(table_name)
            .bind(table_schema)
            .fetch_all(pool)
            .await?;

        Ok(rows
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
            .collect())
    }

    async fn fetch_primary_keys(
        &self,
        table_name: &str,
        table_schema: &str,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<String>> {
        let query = r#"
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

        let rows = sqlx::query(query)
            .bind(table_name)
            .bind(table_schema)
            .fetch_all(pool)
            .await?;

        Ok(rows.into_iter().map(|row| row.get("column_name")).collect())
    }

    async fn fetch_foreign_keys(
        &self,
        table_name: &str,
        table_schema: &str,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<ForeignKeyInfo>> {
        let query = r#"
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
                foreign_table_schema: row.get("foreign_table_schema"),
                foreign_table_name: row.get("foreign_table_name"),
                foreign_column_name: row.get("foreign_column_name"),
            })
            .collect())
    }

    async fn fetch_indexes(
        &self,
        table_name: &str,
        table_schema: &str,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<IndexInfo>> {
        let query = r#"
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

        let rows = sqlx::query(query)
            .bind(table_name)
            .bind(table_schema)
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| IndexInfo {
                index_name: row.get("index_name"),
                columns: row.get("columns"),
                is_unique: row.get("is_unique"),
                is_primary: row.get("is_primary"),
                index_type: row.get("index_type"),
            })
            .collect())
    }

    async fn fetch_constraints(
        &self,
        table_name: &str,
        table_schema: &str,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<ConstraintInfo>> {
        let query = r#"
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

        let rows = sqlx::query(query)
            .bind(table_name)
            .bind(table_schema)
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| ConstraintInfo {
                constraint_name: row.get("constraint_name"),
                constraint_type: row.get("constraint_type"),
                columns: row.get("columns"),
                check_clause: row.get("check_clause"),
            })
            .collect())
    }

    /// Generates a human-readable schema description for LLM consumption
    pub fn format_schema_for_llm(&self, schema: &DatabaseSchema) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "# Database Schema ({} tables)\n\n",
            schema.total_tables
        ));

        for table in &schema.tables {
            self.format_table_for_llm(table, &mut output);
        }

        output
    }

    fn format_table_for_llm(&self, table: &TableSchema, output: &mut String) {
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
            self.format_column_for_llm(col, output);
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
                    .copied()
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

    fn format_column_for_llm(&self, col: &ColumnDetail, output: &mut String) {
        let nullable = if col.is_nullable { "NULL" } else { "NOT NULL" };
        let mut col_line = format!("- **{}**: {} {}", col.column_name, col.data_type, nullable);

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
}
