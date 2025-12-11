//! SQL editing support module.
//!
//! This module provides:
//! - `analyzer` - SQL query detection and parsing with tree-sitter
//! - `completions` - LSP-style completion provider for SQL
//! - `completion_agent` - Agent-powered inline completions

mod analyzer;
mod completion_agent;
mod completions;

pub use analyzer::{SqlQuery, SqlQueryAnalyzer};
pub use completion_agent::handle_completion_requests;
pub use completions::SqlCompletionProvider;
