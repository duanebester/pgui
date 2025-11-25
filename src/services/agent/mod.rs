//! Agent module for LLM-powered assistant functionality.
//!
//! This module provides:
//! - `client` - The Agent client for communicating with Anthropic's API
//! - `messages` - Request/response types and UI message types
//! - `types` - Core types like Tool, Message, ContentBlock

mod client;
mod messages;
mod types;

// Re-export main client types
#[allow(unused_imports)]
pub use client::{Agent, AgentBuilder, create_get_schema_tool};

// Re-export message types
#[allow(unused_imports)]
pub use messages::{
    AgentRequest, AgentResponse, MessageMetadata, MessageRole, ToolCallData, ToolResultData,
    UiMessage,
};

// Re-export core types
#[allow(unused_imports)]
pub use types::{ContentBlock, Message, PropertySchema, Tool, ToolDefinition, create_input_schema};
