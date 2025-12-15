use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use gpui::*;
use gpui_component::input::{CompletionProvider, InputState, Rope, RopeExt};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionResponse, CompletionTextEdit,
    InlineCompletionContext, InlineCompletionItem, InlineCompletionResponse, InsertReplaceEdit,
    InsertTextFormat,
};

use crate::services::{
    agent::Agent,
    sql::completion_agent::{build_completion_agent, build_completion_prompt, get_completion},
};
use crate::{services::agent::InlineCompletionRequest, state::EditorInlineCompletions};

/// Default debounce duration for inline completions.
const DEFAULT_INLINE_COMPLETION_DEBOUNCE: Duration = Duration::from_millis(600);

/// SQL completion provider that implements LSP-style completions
/// with optional agent-powered inline completions
#[derive(Clone)]
pub struct SqlCompletionProvider {
    completions: Arc<RwLock<Vec<CompletionItem>>>,
    agent: Option<Agent>,
    schema: Arc<RwLock<Option<String>>>,
    /// Counter for generating unique request IDs
    request_counter: Arc<AtomicU64>,
    /// Track the latest request ID to ignore stale responses
    latest_request_id: Arc<AtomicU64>,
    inline_completions_enabled: Arc<AtomicBool>,
}

impl SqlCompletionProvider {
    pub fn new() -> Self {
        let completions =
            serde_json::from_slice::<Vec<CompletionItem>>(include_bytes!("./completions.json"))
                .unwrap();

        let agent = build_completion_agent();

        Self {
            agent,
            schema: Arc::new(RwLock::new(None)),
            completions: Arc::new(RwLock::new(completions)),
            request_counter: Arc::new(AtomicU64::new(0)),
            latest_request_id: Arc::new(AtomicU64::new(0)),
            inline_completions_enabled: Arc::new(AtomicBool::new(false)),
        }
    }

    fn get_completions(&self) -> Vec<CompletionItem> {
        let guard = self.completions.read().unwrap();
        guard.clone()
    }

    pub fn toggle_inline_completions(&self, enabled: bool) {
        self.inline_completions_enabled
            .store(enabled, Ordering::SeqCst);
    }

    fn get_inline_completions_enabled(&self) -> bool {
        let guard = self.inline_completions_enabled.load(Ordering::SeqCst);
        guard.clone()
    }

    /// Adds schema-derived completions (table names, column names, etc.)
    pub fn add_schema_completions(&self, completions: Vec<CompletionItem>) {
        let mut guard = self.completions.write().unwrap();
        guard.extend(completions);
    }

    pub fn add_schema(&self, schema: String) {
        let mut guard = self.schema.write().unwrap();
        *guard = Some(schema);
    }

    fn get_schema(&self) -> Option<String> {
        let guard = self.schema.read().unwrap();
        guard.clone()
    }

    /// Generate a new unique request ID
    fn next_request_id(&self) -> u64 {
        self.request_counter.fetch_add(1, Ordering::SeqCst)
    }
}

fn empty_response() -> InlineCompletionResponse {
    InlineCompletionResponse::Array(vec![])
}

fn suggestion_response(text: String) -> InlineCompletionResponse {
    InlineCompletionResponse::Array(vec![InlineCompletionItem {
        insert_text: text,
        filter_text: None,
        range: None,
        command: None,
        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
    }])
}

impl CompletionProvider for SqlCompletionProvider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        trigger: CompletionContext,
        _: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<CompletionResponse>> {
        let trigger_character = trigger.trigger_character.unwrap_or_default();
        if trigger_character.is_empty() {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        }

        // Slash commands can trigger anywhere
        if trigger_character.starts_with("/") {
            let rope = rope.clone();
            return cx.background_spawn(async move {
                let items = build_slash_completions(&rope, offset, &trigger_character);
                Ok(CompletionResponse::Array(items))
            });
        }

        // For regular completions, only trigger at word boundaries
        // offset points to after the trigger character, so we check offset - 2
        // to see what character was before the trigger
        if offset > trigger_character.len() {
            let prev_char_offset = offset - trigger_character.len() - 1;
            let prev_char = rope
                .slice(prev_char_offset..prev_char_offset + 1)
                .to_string();
            if let Some(ch) = prev_char.chars().next() {
                // If previous char is alphanumeric or underscore, we're mid-word - skip
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    return Task::ready(Ok(CompletionResponse::Array(vec![])));
                }
            }
        }

        let items = self.get_completions();
        cx.background_spawn(async move {
            let items = items
                .iter()
                .filter(|item| item.label.starts_with(&trigger_character))
                .take(10)
                .map(|item| {
                    let mut item = item.clone();
                    item.insert_text = Some(item.label.replace(&trigger_character, ""));
                    item
                })
                .collect::<Vec<_>>();

            Ok(CompletionResponse::Array(items))
        })
    }

    #[inline]
    fn inline_completion_debounce(&self) -> Duration {
        DEFAULT_INLINE_COMPLETION_DEBOUNCE
    }

    fn inline_completion(
        &self,
        rope: &Rope,
        offset: usize,
        _trigger: InlineCompletionContext,
        _window: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<InlineCompletionResponse>> {
        if !self.get_inline_completions_enabled() {
            return Task::ready(Ok(InlineCompletionResponse::Array(vec![])));
        }
        if self.agent.is_none() {
            return Task::ready(Ok(InlineCompletionResponse::Array(vec![])));
        }

        cx.update_global::<EditorInlineCompletions, _>(|eic, _cx| {
            eic.loading = true;
        });

        let rope = rope.clone();
        let request_id = self.next_request_id();
        let _latest_request_id = self.latest_request_id.clone();

        let mut agent = self.agent.clone().unwrap();
        let schema = self.get_schema().clone();

        let task = cx.spawn(async move |_this, cx| {
            let res = cx
                .background_spawn(async move {
                    let point = rope.offset_to_point(offset);
                    let line_start = rope.line_start_offset(point.row);
                    let line_end = rope.line_end_offset(point.row);
                    let _current_line = rope.slice(line_start..offset).to_string();

                    let prefix = rope.slice(line_start..offset).to_string();
                    let suffix = rope.slice(offset..line_end).to_string();

                    // Include up to 10 previous lines as context
                    let context = (point.row > 0).then(|| {
                        let ctx_start = rope.line_start_offset(point.row.saturating_sub(10));
                        rope.slice(ctx_start..line_start).to_string()
                    });

                    let request = InlineCompletionRequest {
                        request_id,
                        prefix: prefix,
                        suffix: suffix,
                        context: context,
                    };
                    let prompt = build_completion_prompt(&request, &schema);
                    let suggestion = get_completion(&mut agent, prompt).await;

                    Ok(suggestion
                        .map(suggestion_response)
                        .unwrap_or_else(empty_response))
                })
                .await;

            let _ = cx.update_global::<EditorInlineCompletions, _>(|eic, _cx| {
                eic.loading = false;
            });

            res
        });

        task
    }

    fn is_completion_trigger(
        &self,
        _offset: usize,
        new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        let Some(ch) = new_text.chars().next() else {
            return false;
        };

        // Only trigger for word-starting characters or slash commands
        ch.is_ascii_alphabetic() || ch == '_' || ch == '/'
    }
}

/// Builds slash-command completions (e.g., /date, /thanks)
fn build_slash_completions(rope: &Rope, offset: usize, trigger: &str) -> Vec<CompletionItem> {
    let start = offset.saturating_sub(trigger.len());
    let start_pos = rope.offset_to_position(start);
    let end_pos = rope.offset_to_position(offset);
    let replace_range = lsp_types::Range::new(start_pos, end_pos);

    vec![
        completion_item(
            &replace_range,
            "/date",
            &chrono::Local::now().date_naive().to_string(),
            "Insert current date",
        ),
        completion_item(&replace_range, "/thanks", "Thank you!", "Insert Thank you!"),
        completion_item(&replace_range, "/+1", "👍", "Insert 👍"),
        completion_item(&replace_range, "/-1", "👎", "Insert 👎"),
        completion_item(&replace_range, "/smile", "😊", "Insert 😊"),
        completion_item(&replace_range, "/sad", "😢", "Insert 😢"),
        completion_item(&replace_range, "/launch", "🚀", "Insert 🚀"),
    ]
}

fn completion_item(range: &lsp_types::Range, label: &str, text: &str, doc: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(lsp_types::CompletionItemKind::FUNCTION),
        text_edit: Some(CompletionTextEdit::InsertAndReplace(InsertReplaceEdit {
            new_text: text.to_string(),
            insert: *range,
            replace: *range,
        })),
        documentation: Some(lsp_types::Documentation::String(doc.to_string())),
        insert_text: None,
        ..Default::default()
    }
}
