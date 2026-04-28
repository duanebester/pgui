//! MySQL backend implementation. Targets MySQL 8.4 LTS but is wire- and
//! `information_schema`-compatible with the 8.0 series as well.

pub mod query;
pub mod schema;
