//! Agent-powered inline completion handler.

// use crate::services::TableInfo;
use crate::services::agent::{Agent, ContentBlock, InlineCompletionRequest};

pub const COMPLETION_SYSTEM_PROMPT: &str = r#"You are a SQL completion assistant. Your task is to complete SQL code based on the given prefix and db schema.

RULES:
1. Return the completion text - no explanations, no markdown, no quotes
2. Complete the current statement naturally, prepent a space or newline if it makes sense (e.g. newline if prefix is a comment).
3. If the prefix ends with "--", suggest a brief, helpful comment
4. If completing a keyword, match the case style of the prefix
5. Keep suggestions concise, similar to any context, and performant.
6. If you cannot provide a meaningful completion, return an empty string
"#;

pub fn build_completion_agent() -> Option<Agent> {
    let agent = match Agent::builder()
        .system_prompt(COMPLETION_SYSTEM_PROMPT.to_string())
        .model("claude-haiku-4-5-20251001".to_string())
        .max_tokens(1024)
        .build(vec![])
    {
        Ok(agent) => Some(agent),
        Err(e) => {
            tracing::error!("Failed to create completion agent: {}", e);
            None
        }
    };
    agent
}

pub async fn get_completion(agent: &mut Agent, prompt: String) -> Option<String> {
    match agent
        .chat_step(vec![ContentBlock::Text { text: prompt }])
        .await
    {
        Ok(crate::services::agent::AgentResponse::TextResponse { text, .. }) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        _ => None,
    }
}

pub fn build_completion_prompt(req: &InlineCompletionRequest, schema: &Option<String>) -> String {
    let mut prompt = format!("Complete this SQL:\n{}", req.prefix);

    if !req.suffix.is_empty() {
        prompt.push_str(&format!("[cursor]{}", req.suffix));
    }

    if let Some(context) = &req.context {
        prompt.push_str(&format!("\n\nPrevious lines:\n{}", context));
    }

    // Add table info from the database
    if schema.is_some() {
        prompt.push_str(&format!("\n\nSchema: {}\n", schema.as_ref().unwrap()));
    }

    prompt
}
