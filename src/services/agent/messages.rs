//! Message types for agent communication and UI display.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// Agent Communication Types
// ============================================================================

/// Messages sent from UI to Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentRequest {
    /// Start a new chat with a user message
    Chat(String),
    /// Provide results for tool calls
    ToolResults(Vec<ToolResultData>),
    /// Clear conversation history
    ClearHistory,
    /// Change the LLM model
    SetModel(String),
}

/// Messages sent from Agent to UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentResponse {
    /// Agent needs tools to be executed (with optional accompanying text)
    ToolCallRequest {
        text: Option<String>,
        tool_calls: Vec<ToolCallData>,
        stop_reason: Option<String>,
    },
    /// Agent has a text response ready
    TextResponse {
        text: String,
        stop_reason: Option<String>,
    },
    /// Agent encountered an error
    Error(String),
}

impl AgentResponse {
    /// Check if this response indicates the agent is done processing
    pub fn is_done(&self) -> bool {
        match self {
            AgentResponse::ToolCallRequest { stop_reason, .. } => {
                // If stop_reason is "end_turn" or "max_tokens", we're done
                // If it's "tool_use" or None, we're still processing
                matches!(
                    stop_reason.as_deref(),
                    Some("end_turn") | Some("max_tokens")
                )
            }
            AgentResponse::TextResponse { stop_reason, .. } => {
                // Text responses usually mean done, but check stop_reason to be sure
                matches!(
                    stop_reason.as_deref(),
                    Some("end_turn") | Some("max_tokens") | None
                )
            }
            AgentResponse::Error(_) => true, // Errors always end the processing
        }
    }
}

/// Data for a tool call that needs to be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallData {
    pub id: String,
    pub name: String,
    pub input: Value,
}

/// Data for a tool result being returned to the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultData {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

// ============================================================================
// UI Message Types
// ============================================================================

/// Role of a message in the UI conversation display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    ToolCall,
    ToolResult,
}

/// A message in the UI conversation display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<MessageMetadata>,
}

/// Additional metadata for messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub tool_name: Option<String>,
    pub is_error: bool,
    pub tool_input: Option<Value>,
}

impl UiMessage {
    /// Create a new user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create a new assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create a new tool call message
    pub fn tool_call(tool_name: String, tool_input: Value) -> Self {
        Self {
            role: MessageRole::ToolCall,
            content: format!("Calling {}", tool_name),
            timestamp: Utc::now(),
            metadata: Some(MessageMetadata {
                tool_name: Some(tool_name),
                is_error: false,
                tool_input: Some(tool_input),
            }),
        }
    }

    /// Create an error message
    pub fn error(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: format!("❌ Error: {}", content.into()),
            timestamp: Utc::now(),
            metadata: Some(MessageMetadata {
                tool_name: None,
                is_error: true,
                tool_input: None,
            }),
        }
    }
}
