//! Agent client for communicating with the Anthropic API.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

use super::messages::{AgentResponse, ToolCallData, ToolResultData};
use super::types::{
    ContentBlock, Message, PropertySchema, Tool, ToolDefinition, create_input_schema,
};

/// Agent that can converse with an LLM and execute tools
#[derive(Clone)]
pub struct Agent {
    api_key: String,
    model: String,
    system_prompt: String,
    tools: Vec<Tool>,
    conversation: Vec<Message>,
    max_tokens: u32,
}

// Anthropic API request/response types
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: Usage,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

#[allow(dead_code)]
impl Agent {
    /// Create a new agent with the given tools
    pub fn new(tools: Vec<Tool>) -> Result<Self> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow!("ANTHROPIC_API_KEY environment variable not set"))?;

        Ok(Self {
            api_key,
            model: "claude-haiku-4-5-20251001".to_string(),
            system_prompt: Self::default_system_prompt(),
            tools,
            conversation: Vec::new(),
            max_tokens: 4096,
        })
    }

    /// Create a new agent with custom configuration
    pub fn builder() -> AgentBuilder {
        AgentBuilder::default()
    }

    /// Default system prompt
    fn default_system_prompt() -> String {
        "You are a helpful AI assistant with access to tools that can help you complete tasks. \
        When you need to use a tool, respond with the appropriate tool call. \
        Be concise and helpful in your responses."
            .to_string()
    }

    /// Set the system prompt
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = prompt;
    }

    /// Set the model
    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    /// Set max tokens
    pub fn set_max_tokens(&mut self, max_tokens: u32) {
        self.max_tokens = max_tokens;
    }

    /// Add a user message to the conversation
    pub fn add_user_message(&mut self, content: String) {
        self.conversation.push(Message::User {
            role: "user".to_string(),
            content: vec![ContentBlock::Text { text: content }],
        });
    }

    /// Add an assistant message to the conversation
    fn add_assistant_message(&mut self, content: Vec<ContentBlock>) {
        self.conversation.push(Message::Assistant {
            role: "assistant".to_string(),
            content,
        });
    }

    /// Get all tool definitions in a format suitable for the LLM
    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(),
            })
            .collect()
    }

    /// Process a single step in the conversation
    /// Returns either tool calls that need execution, or a final text response
    pub async fn chat_step(&mut self, user_content: Vec<ContentBlock>) -> Result<AgentResponse> {
        // Add user message
        self.conversation.push(Message::User {
            role: "user".to_string(),
            content: user_content,
        });

        // Run inference in a blocking task since smolhttp is synchronous
        let mut agent_clone = self.clone_for_inference();
        let response = smol::unblock(move || agent_clone.run_inference()).await?;

        println!("Usage: {:?}", response.usage);
        println!("Stop reason: {:?}", response.stop_reason);
        println!("Model: {}", response.model);

        // Add assistant response to conversation
        self.add_assistant_message(response.content.clone());

        // Parse the response content
        let mut tool_calls = Vec::new();
        let mut text_response = String::new();

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    text_response = text.clone();
                    println!("ContentBlock::Text: {}", text);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    println!("ContentBlock::ToolUse: {}, {}, {}", id, name, input);
                    tool_calls.push(ToolCallData {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                }
                ContentBlock::ToolResult { .. } => {
                    println!("ContentBlock::ToolResult: shouldn't happen");
                }
            }
        }

        if !tool_calls.is_empty() {
            return Ok(AgentResponse::ToolCallRequest {
                text: if text_response.is_empty() {
                    None
                } else {
                    Some(text_response)
                },
                tool_calls,
                stop_reason: response.stop_reason,
            });
        }

        if !text_response.is_empty() {
            return Ok(AgentResponse::TextResponse {
                text: text_response,
                stop_reason: response.stop_reason,
            });
        }

        Err(anyhow!("No text or tool calls in assistant response"))
    }

    /// Submit tool results back to the agent
    pub fn submit_tool_results(&mut self, results: Vec<ToolResultData>) {
        let content_blocks: Vec<ContentBlock> = results
            .into_iter()
            .map(|result| ContentBlock::ToolResult {
                tool_use_id: result.tool_use_id,
                content: result.content,
                is_error: Some(result.is_error),
            })
            .collect();

        self.conversation.push(Message::User {
            role: "user".to_string(),
            content: content_blocks,
        });
    }

    /// Clone the agent state needed for inference (without tools)
    fn clone_for_inference(&self) -> AgentForInference {
        AgentForInference {
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            system_prompt: self.system_prompt.clone(),
            tool_definitions: self.get_tool_definitions(),
            conversation: self.conversation.clone(),
            max_tokens: self.max_tokens,
        }
    }

    /// Get the current conversation history
    #[allow(dead_code)]
    pub fn get_conversation(&self) -> &[Message] {
        &self.conversation
    }

    /// Clear the conversation history
    pub fn clear_conversation(&mut self) {
        self.conversation.clear();
    }
}

/// A lightweight version of Agent for running inference without tool execution
struct AgentForInference {
    api_key: String,
    model: String,
    system_prompt: String,
    tool_definitions: Vec<ToolDefinition>,
    conversation: Vec<Message>,
    max_tokens: u32,
}

impl AgentForInference {
    fn run_inference(&mut self) -> Result<AnthropicResponse> {
        let tool_defs = if self.tool_definitions.is_empty() {
            None
        } else {
            Some(self.tool_definitions.clone())
        };

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: self.conversation.clone(),
            system: Some(self.system_prompt.clone()),
            tools: tool_defs,
        };

        let body = serde_json::to_string(&request)
            .map_err(|e| anyhow!("Failed to serialize request: {}", e))?;

        let response = smolhttp::Client::new("https://api.anthropic.com/v1/messages")
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?
            .post()
            .headers(vec![
                ("x-api-key".to_string(), self.api_key.clone()),
                ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ("content-type".to_string(), "application/json".to_string()),
            ])
            .body(body.into())
            .send()
            .map_err(|e| anyhow!("API request failed: {}", e))?;

        let response_text = response.text();

        if response_text.contains("\"error\"") && response_text.contains("\"type\"") {
            return Err(anyhow!("API error: {}", response_text));
        }

        let api_response: AnthropicResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                anyhow!(
                    "Failed to parse response: {}. Response: {}",
                    e,
                    response_text
                )
            })?;

        Ok(api_response)
    }
}

/// Builder for creating agents with custom configuration
pub struct AgentBuilder {
    api_key: Option<String>,
    model: String,
    system_prompt: String,
    max_tokens: u32,
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self {
            api_key: None,
            model: "claude-haiku-4-5-20251001".to_string(),
            system_prompt: Agent::default_system_prompt(),
            max_tokens: 4096,
        }
    }
}

#[allow(dead_code)]
impl AgentBuilder {
    pub fn api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    pub fn system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = prompt;
        self
    }

    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn build(self, tools: Vec<Tool>) -> Result<Agent> {
        let api_key = match self.api_key {
            Some(key) => key,
            None => env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow!("ANTHROPIC_API_KEY environment variable not set"))?,
        };

        Ok(Agent {
            api_key,
            model: self.model,
            system_prompt: self.system_prompt,
            tools,
            conversation: Vec::new(),
            max_tokens: self.max_tokens,
        })
    }
}

// ============================================================================
// Tool Helpers
// ============================================================================

/// Example tool: Get formatted schema as markdown
pub fn create_get_schema_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "table_name".to_string(),
        PropertySchema {
            property_type: "string".to_string(),
            description: "The name of the database table to get schema for".to_string(),
        },
    );

    Tool {
        name: "get_schema".to_string(),
        description: "Get the schema for a database table formatted as markdown".to_string(),
        input_schema: create_input_schema(properties, vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_builder() {
        let agent = Agent::builder()
            .api_key("test-key".to_string())
            .model("claude-sonnet-4.5-20250929".to_string())
            .system_prompt("You are a test assistant".to_string())
            .max_tokens(2048)
            .build(vec![create_get_schema_tool()]);

        assert!(agent.is_ok());
    }

    #[test]
    fn test_tool_definitions() {
        let agent = Agent::builder()
            .api_key("test-key".to_string())
            .build(vec![create_get_schema_tool()])
            .unwrap();

        let defs = agent.get_tool_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "get_schema");
    }
}
