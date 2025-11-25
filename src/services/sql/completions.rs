use gpui::*;
use gpui_component::input::{CompletionProvider, InputState, Rope, RopeExt};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionResponse, CompletionTextEdit, InsertReplaceEdit,
};
use std::sync::{Arc, RwLock};

/// SQL completion provider that implements LSP-style completions
#[derive(Clone)]
pub struct SqlCompletionProvider {
    completions: Arc<RwLock<Vec<CompletionItem>>>,
}

impl SqlCompletionProvider {
    pub fn new() -> Self {
        let completions =
            serde_json::from_slice::<Vec<CompletionItem>>(include_bytes!("./completions.json"))
                .unwrap();

        Self {
            completions: Arc::new(RwLock::new(completions)),
        }
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

    fn is_completion_trigger(
        &self,
        _offset: usize,
        _new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        true
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
