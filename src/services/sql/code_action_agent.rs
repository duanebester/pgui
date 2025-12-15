//! Agent-powered SQL code action provider.
//!
//! Unlike inline completions which are automatic and need to be fast,
//! code actions are user-initiated and can be more comprehensive.

use std::ops::Range;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use gpui::{App, BorrowAppContext as _, Entity, SharedString, Task, Window};
use gpui_component::input::{CodeActionProvider, InputState, RopeExt};
use lsp_types::{CodeAction, CodeActionKind, TextEdit};

use crate::services::agent::{Agent, AgentResponse, ContentBlock};
use crate::state::EditorCodeActions;

/// System prompt for SQL code actions
const CODE_ACTION_SYSTEM_PROMPT: &str = r#"You are a SQL assistant. The user has explicitly requested your help with their SQL query.

RULES:
1. Return ONLY the requested content - no markdown code fences, no explanations unless asked
2. Use the provided schema to suggest correct table and column names
3. Match the existing code style (uppercase/lowercase keywords, indentation)
4. Be concise but complete
"#;

/// Types of AI actions available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionType {
    Complete,
    Explain,
    Optimize,
}

impl ActionType {
    fn title(&self) -> &'static str {
        match self {
            ActionType::Complete => "AI: Complete SQL",
            ActionType::Explain => "AI: Explain SQL",
            ActionType::Optimize => "AI: Optimize SQL",
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            ActionType::Complete => "complete",
            ActionType::Explain => "explain",
            ActionType::Optimize => "optimize",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "complete" => Some(ActionType::Complete),
            "explain" => Some(ActionType::Explain),
            "optimize" => Some(ActionType::Optimize),
            _ => None,
        }
    }
}

/// SQL Code Action Provider with AI-powered actions
#[derive(Clone)]
pub struct SqlCodeActionProvider {
    agent: Option<Agent>,
    schema: Arc<RwLock<Option<String>>>,
}

impl SqlCodeActionProvider {
    pub fn new() -> Self {
        let agent = build_code_action_agent();
        Self {
            agent,
            schema: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_schema(&self, schema: String) {
        let mut guard = self.schema.write().unwrap();
        *guard = Some(schema);
    }

    fn get_schema(&self) -> Option<String> {
        self.schema.read().unwrap().clone()
    }
}

fn build_code_action_agent() -> Option<Agent> {
    match Agent::builder()
        .system_prompt(CODE_ACTION_SYSTEM_PROMPT.to_string())
        .model("claude-haiku-4-5-20251001".to_string())
        .max_tokens(2048)
        .build(vec![])
    {
        Ok(agent) => Some(agent),
        Err(e) => {
            tracing::error!("Failed to create code action agent: {}", e);
            None
        }
    }
}

fn build_prompt(action: ActionType, sql: &str, schema: &Option<String>) -> String {
    let mut prompt = String::new();

    match action {
        ActionType::Complete => {
            prompt.push_str("Complete this SQL query at [CURSOR]. Return ONLY raw SQL to insert - no markdown, no code fences, no explanations.\n\n");
            prompt.push_str(sql);
        }
        ActionType::Explain => {
            prompt.push_str(
                "Explain what this SQL query does in plain English. Be concise but thorough.\n\n",
            );
            prompt.push_str(sql);
        }
        ActionType::Optimize => {
            prompt.push_str("Optimize this SQL query for better performance. Return ONLY raw SQL - no markdown, no code fences, no explanations.\n\n");
            prompt.push_str(sql);
        }
    }

    if let Some(s) = schema {
        prompt.push_str(&format!("\n\nDatabase schema:\n{}", s));
    }

    prompt
}

impl CodeActionProvider for SqlCodeActionProvider {
    fn id(&self) -> SharedString {
        "SqlAIAssist".into()
    }

    fn code_actions(
        &self,
        state: Entity<InputState>,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Vec<CodeAction>>> {
        if self.agent.is_none() {
            return Task::ready(Ok(vec![]));
        }

        // Check if there's any SQL content to work with
        let has_content = {
            let input = state.read(cx);
            !input.text().to_string().trim().is_empty()
        };

        let has_selection = range.start != range.end;

        let mut actions = vec![];

        // Always offer Complete (works at cursor)
        actions.push(CodeAction {
            title: ActionType::Complete.title().into(),
            kind: Some(CodeActionKind::REFACTOR),
            edit: None,
            data: Some(serde_json::json!({
                "type": ActionType::Complete.as_str(),
                "range_start": range.start,
                "range_end": range.end
            })),
            ..Default::default()
        });

        // Offer Explain and Optimize if there's content or selection
        if has_content || has_selection {
            actions.push(CodeAction {
                title: ActionType::Explain.title().into(),
                kind: Some(CodeActionKind::EMPTY),
                edit: None,
                data: Some(serde_json::json!({
                    "type": ActionType::Explain.as_str(),
                    "range_start": range.start,
                    "range_end": range.end
                })),
                ..Default::default()
            });

            actions.push(CodeAction {
                title: ActionType::Optimize.title().into(),
                kind: Some(CodeActionKind::REFACTOR),
                edit: None,
                data: Some(serde_json::json!({
                    "type": ActionType::Optimize.as_str(),
                    "range_start": range.start,
                    "range_end": range.end
                })),
                ..Default::default()
            });
        }

        Task::ready(Ok(actions))
    }

    fn perform_code_action(
        &self,
        state: Entity<InputState>,
        action: CodeAction,
        _push_to_history: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let data = match &action.data {
            Some(d) => d.clone(),
            None => return Task::ready(Ok(())),
        };

        let action_type = data
            .get("type")
            .and_then(|t| t.as_str())
            .and_then(ActionType::from_str);

        let Some(action_type) = action_type else {
            return Task::ready(Ok(()));
        };

        // Get the range from the action data
        let range_start = data
            .get("range_start")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let range_end = data.get("range_end").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let selection_range = range_start..range_end;

        let Some(mut agent) = self.agent.clone() else {
            return Task::ready(Ok(()));
        };

        cx.update_global::<EditorCodeActions, _>(|eca, _cx| {
            eca.loading = true;
        });

        let schema = self.get_schema();
        let state_weak = state.downgrade();

        // Spawn async task - do ALL state reading inside update_in
        window.spawn(cx, async move |cx| {
            // First, read the state to build the prompt
            let prompt_data = state_weak.update_in(cx, |input, _window, _cx| {
                let text = input.text();
                let cursor = input.cursor();
                let text_len = text.len();

                let sql_content = if selection_range.start != selection_range.end {
                    text.slice(selection_range.clone()).to_string()
                } else {
                    text.to_string()
                };

                let sql_for_prompt = if action_type == ActionType::Complete {
                    let before = text.slice(0..cursor).to_string();
                    let after = text.slice(cursor..text_len).to_string();
                    format!("{}[CURSOR]{}", before, after)
                } else {
                    sql_content.clone()
                };

                (sql_for_prompt, cursor, text_len)
            })?;

            let (sql_for_prompt, cursor_offset, _text_len) = prompt_data;
            let prompt = build_prompt(action_type, &sql_for_prompt, &schema);

            // Call the AI
            let result = match agent
                .chat_step(vec![ContentBlock::Text { text: prompt }])
                .await
            {
                Ok(AgentResponse::TextResponse { text, .. }) => {
                    let cleaned = strip_code_fences(&text);
                    if cleaned.is_empty() {
                        return Ok(());
                    }
                    cleaned.to_string()
                }
                Ok(_) => return Ok(()),
                Err(e) => {
                    tracing::error!("AI action failed: {}", e);
                    return Ok(());
                }
            };

            // Apply the result
            match action_type {
                ActionType::Complete => {
                    state_weak.update_in(cx, |input, window, cx| {
                        let pos = input.text().offset_to_position(cursor_offset);
                        let range = lsp_types::Range::new(pos, pos);
                        input.apply_lsp_edits(
                            &vec![TextEdit {
                                range,
                                new_text: result,
                                ..Default::default()
                            }],
                            window,
                            cx,
                        );
                    })?;
                }
                ActionType::Explain => {
                    state_weak.update_in(cx, |input, window, cx| {
                        let comment = result
                            .lines()
                            .map(|line| format!("-- {}", line))
                            .collect::<Vec<_>>()
                            .join("\n");

                        let insert_pos = if selection_range.start != selection_range.end {
                            selection_range.start
                        } else {
                            0
                        };

                        let pos = input.text().offset_to_position(insert_pos);
                        let range = lsp_types::Range::new(pos, pos);
                        input.apply_lsp_edits(
                            &vec![TextEdit {
                                range,
                                new_text: format!("{}\n", comment),
                                ..Default::default()
                            }],
                            window,
                            cx,
                        );
                    })?;
                }
                ActionType::Optimize => {
                    state_weak.update_in(cx, |input, window, cx| {
                        let current_len = input.text().len();
                        let (start, end) = if selection_range.start != selection_range.end {
                            (selection_range.start, selection_range.end)
                        } else {
                            (0, current_len)
                        };

                        let start_pos = input.text().offset_to_position(start);
                        let end_pos = input.text().offset_to_position(end);
                        let range = lsp_types::Range::new(start_pos, end_pos);

                        input.apply_lsp_edits(
                            &vec![TextEdit {
                                range,
                                new_text: result,
                                ..Default::default()
                            }],
                            window,
                            cx,
                        );
                    })?;
                }
            }

            let _ = cx.update_global::<EditorCodeActions, _>(|eca, _win, _cx| {
                eca.loading = false;
            });

            Ok(())
        })
    }
}

/// Strip markdown code fences from AI response
fn strip_code_fences(text: &str) -> String {
    let trimmed = text.trim();

    // Check for ```sql or ``` at start
    let without_start = if trimmed.starts_with("```sql") {
        trimmed
            .strip_prefix("```sql")
            .unwrap_or(trimmed)
            .trim_start()
    } else if trimmed.starts_with("```") {
        trimmed.strip_prefix("```").unwrap_or(trimmed).trim_start()
    } else {
        trimmed
    };

    // Check for ``` at end
    let without_end = if without_start.ends_with("```") {
        without_start
            .strip_suffix("```")
            .unwrap_or(without_start)
            .trim_end()
    } else {
        without_start
    };

    without_end.to_string()
}
