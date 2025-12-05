//! Core types for the agent module.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A tool that can be executed by the agent
#[derive(Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Message in a conversation with the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    User {
        role: String,
        content: Vec<ContentBlock>,
    },
    Assistant {
        role: String,
        content: Vec<ContentBlock>,
    },
}

/// Content block within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    #[serde(rename = "document")]
    Document { source: FileSource },
}

/// File source for referencing uploaded files
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FileSource {
    #[serde(rename = "file")]
    File { file_id: String },
}

/// Tool definition for the LLM API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}
