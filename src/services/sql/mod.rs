//! SQL editing support module.
//!
//! This module provides:
//! - `analyzer` - SQL query detection and parsing with tree-sitter
//! - `completions` - LSP-style completion provider for SQL
//! - `completion_agent` - Agent-powered inline completions
//! - `code_action_agent` - Agent-powered code actions (Complete, Explain, Optimize)

mod analyzer;
mod code_action_agent;
mod completion_agent;
mod completions;

pub use analyzer::{SqlQuery, SqlQueryAnalyzer};
pub use code_action_agent::SqlCodeActionProvider;
pub use completions::SqlCompletionProvider;
