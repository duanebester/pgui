use gpui::*;

use gpui_component::input::{CompletionProvider, InputState, Rope, RopeExt};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionResponse, CompletionTextEdit, InsertReplaceEdit,
};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct LspStore {
    completions: Arc<RwLock<Vec<CompletionItem>>>,
}

impl LspStore {
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

    pub fn add_schema_completions(&self, completions: Vec<CompletionItem>) {
        let mut guard = self.completions.write().unwrap();
        guard.extend(completions);
    }
}

fn completion_item(
    replace_range: &lsp_types::Range,
    label: &str,
    replace_text: &str,
    documentation: &str,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(lsp_types::CompletionItemKind::FUNCTION),
        text_edit: Some(CompletionTextEdit::InsertAndReplace(InsertReplaceEdit {
            new_text: replace_text.to_string(),
            insert: replace_range.clone(),
            replace: replace_range.clone(),
        })),
        documentation: Some(lsp_types::Documentation::String(documentation.to_string())),
        insert_text: None,
        ..Default::default()
    }
}

impl CompletionProvider for LspStore {
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
        let items = self.get_completions().clone();
        cx.background_spawn(async move {
            if trigger_character.starts_with("/") {
                let start = offset.saturating_sub(trigger_character.len());
                let start_pos = rope.offset_to_position(start);
                let end_pos = rope.offset_to_position(offset);
                let replace_range = lsp_types::Range::new(start_pos, end_pos);

                let items = vec![
                    completion_item(
                        &replace_range,
                        "/date",
                        format!("{}", chrono::Local::now().date_naive()).as_str(),
                        "Insert current date",
                    ),
                    completion_item(&replace_range, "/thanks", "Thank you!", "Insert Thank you!"),
                    completion_item(&replace_range, "/+1", "👍", "Insert 👍"),
                    completion_item(&replace_range, "/-1", "👎", "Insert 👎"),
                    completion_item(&replace_range, "/smile", "😊", "Insert 😊"),
                    completion_item(&replace_range, "/sad", "😢", "Insert 😢"),
                    completion_item(&replace_range, "/launch", "🚀", "Insert 🚀"),
                ];
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
