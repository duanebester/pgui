//! Agent-powered inline completion handler.

use std::env;

use async_channel::{Receiver, Sender};

use crate::services::TableInfo;
use crate::services::agent::{
    Agent, ContentBlock, InlineAgentRequest, InlineAgentResponse, InlineCompletionRequest,
    InlineCompletionResponse,
};

const COMPLETION_SYSTEM_PROMPT: &str = r#"You are a SQL completion assistant. Your task is to complete SQL code based on the given prefix.

RULES:
1. Return ONLY the completion text - no explanations, no markdown, no quotes
2. Complete the current statement naturally
3. If the prefix ends with "--", suggest a brief, helpful comment
4. If completing a keyword, match the case style of the prefix
5. Keep suggestions concise (prefer single line completions)
6. If you cannot provide a meaningful completion, return an empty string
"#;

pub async fn handle_completion_requests(
    tables: Vec<TableInfo>,
    request_rx: Receiver<InlineAgentRequest>,
    response_tx: Sender<InlineAgentResponse>,
) {
    if env::var("ANTHROPIC_API_KEY").is_err() {
        tracing::warn!("ANTHROPIC_API_KEY not set, inline completions disabled");
        drain_requests_without_api_key(request_rx, response_tx).await;
        return;
    }

    let mut agent = match Agent::builder()
        .system_prompt(COMPLETION_SYSTEM_PROMPT.to_string())
        .model("claude-haiku-4-5-20251001".to_string())
        .max_tokens(100)
        .build(vec![])
    {
        Ok(agent) => agent,
        Err(e) => {
            tracing::error!("Failed to create completion agent: {}", e);
            return;
        }
    };

    tracing::info!("Inline completion agent started");

    while let Ok(request) = request_rx.recv().await {
        match request {
            InlineAgentRequest::Chat { content } => {
                if let Err(e) = agent
                    .chat_step(vec![ContentBlock::Text { text: content }])
                    .await
                {
                    tracing::debug!("Failed to add context: {}", e);
                }
            }
            InlineAgentRequest::InlineCompletion(req) => {
                let request_id = req.request_id;
                let prompt = build_completion_prompt(&req, &tables);
                println!("prompt: {:?}", prompt.clone());
                let suggestion = get_completion(&mut agent, prompt).await;
                println!("suggestion: {:?}", suggestion.clone());

                agent.clear_conversation();

                if response_tx
                    .send(InlineAgentResponse::InlineCompletion(
                        InlineCompletionResponse {
                            request_id,
                            suggestion,
                        },
                    ))
                    .await
                    .is_err()
                {
                    println!("failed to reply");
                    break;
                }
            }
        }
    }
}

async fn drain_requests_without_api_key(
    request_rx: Receiver<InlineAgentRequest>,
    response_tx: Sender<InlineAgentResponse>,
) {
    while let Ok(request) = request_rx.recv().await {
        if let InlineAgentRequest::InlineCompletion(req) = request {
            let _ = response_tx
                .send(InlineAgentResponse::InlineCompletion(
                    InlineCompletionResponse {
                        request_id: req.request_id,
                        suggestion: None,
                    },
                ))
                .await;
        }
    }
}

async fn get_completion(agent: &mut Agent, prompt: String) -> Option<String> {
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

fn build_completion_prompt(req: &InlineCompletionRequest, tables: &[TableInfo]) -> String {
    let mut prompt = format!("Complete this SQL:\n{}", req.prefix);

    if !req.suffix.is_empty() {
        prompt.push_str(&format!("[cursor]{}", req.suffix));
    }

    if let Some(context) = &req.context {
        prompt.push_str(&format!("\n\nPrevious lines:\n{}", context));
    }

    // Add table info from the database
    if !tables.is_empty() {
        prompt.push_str("\n\nAvailable tables:\n");
        for table in tables {
            prompt.push_str(&format!(
                "- {}.{} ({})\n",
                table.table_schema, table.table_name, table.table_type
            ));
        }
    }

    prompt
}
