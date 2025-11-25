mod manager;
mod query;
mod schema;
mod types;

pub use manager::DatabaseManager;

#[allow(unused_imports)]
pub use types::{
    ColumnDetail, ConstraintInfo, DatabaseInfo, DatabaseSchema, EnhancedQueryExecutionResult,
    EnhancedQueryResult, ForeignKeyInfo, IndexInfo, ResultCell, ResultColumnMetadata, ResultRow,
    TableInfo, TableSchema,
};

// TableMetadata is internal only
#[allow(unused_imports)]
pub(crate) use types::TableMetadata;
