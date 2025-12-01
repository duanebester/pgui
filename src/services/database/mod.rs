mod manager;
mod query;
mod schema;
mod types;

pub use manager::DatabaseManager;

#[allow(unused_imports)]
pub use types::{
    ColumnDetail, ConstraintInfo, DatabaseInfo, DatabaseSchema, ErrorResult, ForeignKeyInfo,
    IndexInfo, QueryExecutionResult, QueryResult, ResultCell, ResultColumnMetadata, ResultRow,
    TableInfo, TableSchema,
};

// TableMetadata is internal only
#[allow(unused_imports)]
pub(crate) use types::TableMetadata;
