//! SQL editing support module.
//!
//! This module provides:
//! - `analyzer` - SQL query detection and parsing with tree-sitter
//! - `completions` - LSP-style completion provider for SQL

mod analyzer;
mod completions;

pub use completions::SqlCompletionProvider;
