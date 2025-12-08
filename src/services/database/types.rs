use serde::{Deserialize, Serialize};
use sqlx::postgres::types::Oid;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub table_name: String,
    pub table_schema: String,
    pub table_type: String,
}

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
pub struct QueryResult {
    pub columns: Vec<ResultColumnMetadata>,
    pub rows: Vec<ResultRow>,
    pub row_count: usize,
    pub execution_time_ms: u128,
    pub original_query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedResult {
    pub rows_affected: u64,
    pub execution_time_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResult {
    pub message: String,
    pub execution_time_ms: u128,
}

/// Result of an query execution
#[derive(Debug, Clone)]
pub enum QueryExecutionResult {
    Select(QueryResult),
    Modified(ModifiedResult),
    Error(ErrorResult),
}

/// Internal metadata for table/column resolution
pub(crate) struct TableMetadata {
    pub oid_to_table_name: HashMap<Oid, String>,
    pub column_nullable_map: HashMap<(Oid, String), bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub datname: String,
}
