use std::sync::{
    Arc, RwLock,
    atomic::{AtomicU64, Ordering},
};

use async_channel::{Receiver, Sender, unbounded};
use gpui::*;
use gpui_component::input::{CompletionProvider, InputState, Rope, RopeExt};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionResponse, CompletionTextEdit,
    InlineCompletionContext, InlineCompletionItem, InlineCompletionResponse, InsertReplaceEdit,
    InsertTextFormat,
};

use crate::services::{TableInfo, sql::handle_completion_requests};
use crate::{
    services::agent::{InlineAgentRequest, InlineAgentResponse, InlineCompletionRequest},
    state::EditorState,
};

/// SQL completion provider that implements LSP-style completions
/// with optional agent-powered inline completions
#[derive(Clone)]
pub struct SqlCompletionProvider {
    completions: Arc<RwLock<Vec<CompletionItem>>>,
    tables: Arc<RwLock<Vec<TableInfo>>>,
    completion_request_tx: Sender<InlineAgentRequest>,
    completion_response_rx: Receiver<InlineAgentResponse>,
    /// Counter for generating unique request IDs
    request_counter: Arc<AtomicU64>,
    /// Track the latest request ID to ignore stale responses
    latest_request_id: Arc<AtomicU64>,
}

impl SqlCompletionProvider {
    pub fn new(cx: &mut App) -> Self {
        let tables = cx.global::<EditorState>().tables.clone();

        let completions =
            serde_json::from_slice::<Vec<CompletionItem>>(include_bytes!("./completions.json"))
                .unwrap();

        // Create channels for agent communication
        let (request_tx, request_rx) = unbounded::<InlineAgentRequest>();
        let (response_tx, response_rx) = unbounded::<InlineAgentResponse>();

        Self {
            completions: Arc::new(RwLock::new(completions)),
            tables: Arc::new(RwLock::new(vec![])),
            completion_request_tx: request_tx,
            completion_response_rx: response_rx,
            request_counter: Arc::new(AtomicU64::new(0)),
            latest_request_id: Arc::new(AtomicU64::new(0)),
        }
    }

    fn get_inline_completions(&self) -> () {
        // Spawn the completion agent handler
        // Note: This runs on a background thread pool
        smol::spawn(handle_completion_requests(
            self.get_tables(),
            self.completion_request_tx,
            self.completion_response_tx,
        ))
        .detach();
    }

    fn get_completions(&self) -> Vec<CompletionItem> {
        let guard = self.completions.read().unwrap();
        guard.clone()
    }

    /// Adds schema-derived completions (table names, column names, etc.)
    pub fn add_schema_completions(&self, completions: Vec<CompletionItem>) {
        let mut guard = self.completions.write().unwrap();
        guard.extend(completions);
    }

    fn get_tables(&self) -> Vec<TableInfo> {
        let guard = self.tables.read().unwrap();
        guard.clone()
    }

    /// Add tables
    pub fn add_tables(&self, tables: Vec<TableInfo>) {
        let mut guard = self.tables.write().unwrap();
        guard.extend(tables);
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

        let rope = rope.clone();
        let items = self.get_completions();

        cx.background_spawn(async move {
            if trigger_character.starts_with("/") {
                let items = build_slash_completions(&rope, offset, &trigger_character);
                return Ok(CompletionResponse::Array(items));
            }

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

    fn inline_completion(
        &self,
        rope: &Rope,
        offset: usize,
        _trigger: InlineCompletionContext,
        _window: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<InlineCompletionResponse>> {
        println!("inline_completion");
        let rope = rope.clone();
        let request_id = self.next_request_id();
        let request_tx = self.completion_request_tx.clone();
        let response_rx = self.completion_response_rx.clone();
        let latest_request_id = self.latest_request_id.clone();

        cx.background_spawn(async move {
            let point = rope.offset_to_point(offset);
            let line_start = rope.line_start_offset(point.row);
            let line_end = rope.line_end_offset(point.row);

            let prefix = rope.slice(line_start..offset).to_string();
            let suffix = rope.slice(offset..line_end).to_string();

            // Include up to 10 previous lines as context
            let context = (point.row > 0).then(|| {
                let ctx_start = rope.line_start_offset(point.row.saturating_sub(10));
                rope.slice(ctx_start..line_start).to_string()
            });

            // Send request
            let request = InlineCompletionRequest {
                request_id,
                prefix: prefix,
                suffix: suffix,
                context: context,
            };

            if request_tx
                .send(InlineAgentRequest::InlineCompletion(request))
                .await
                .is_err()
            {
                println!("error sending");
                return Ok(empty_response());
            }

            let suggestion = await_response(request_id, &response_rx, &latest_request_id).await;
            Ok(suggestion
                .map(suggestion_response)
                .unwrap_or_else(empty_response))
        })
    }

    fn is_completion_trigger(
        &self,
        _offset: usize,
        _new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        true
    }
}

/// Wait for a matching response from the agent, with timeout and staleness checking
async fn await_response(
    request_id: u64,
    response_rx: &Receiver<InlineAgentResponse>,
    latest_request_id: &AtomicU64,
) -> Option<String> {
    loop {
        let response = response_rx.recv().await.ok();
        println!("await_response request_id: {:?}", request_id);
        println!("await_response response: {:?}", response);
        match response {
            Some(InlineAgentResponse::InlineCompletion(resp)) if resp.request_id == request_id => {
                if latest_request_id.load(Ordering::SeqCst) != request_id {
                    return None;
                }
                return resp.suggestion;
            }
            Some(InlineAgentResponse::Error(e)) => {
                tracing::debug!("Completion error: {}", e);
                return None;
            }
            Some(_) => continue,
            None => return None,
        }
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
