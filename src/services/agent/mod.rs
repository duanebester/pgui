//! Agent module for LLM-powered assistant functionality.
//!
//! This module provides:
//! - `client` - The Agent client for communicating with Anthropic's API
//! - `messages` - Request/response types and UI message types
//! - `types` - Core types like Tool, Message, ContentBlock

mod client;
mod files;
mod messages;
mod types;

// Re-export main client types
#[allow(unused_imports)]
pub use client::{
    Agent, AgentBuilder, create_get_schema_tool, create_get_table_columns_tool,
    create_get_tables_tool,
};

// Re-export files API
#[allow(unused_imports)]
pub use files::upload_file;

// Re-export message types
#[allow(unused_imports)]
pub use messages::{
    AgentRequest, AgentResponse, InlineAgentRequest, InlineAgentResponse, InlineCompletionRequest,
    InlineCompletionResponse, MessageMetadata, MessageRole, ToolCallData, ToolResultData,
    UiMessage,
};

// Re-export core types
#[allow(unused_imports)]
pub use types::{ContentBlock, FileSource, Message, Tool, ToolDefinition};
