use std::rc::Rc;

use crate::{services::LspStore, state::EditorState};
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState, TabSize},
    v_flex,
};
use lsp_types::CompletionItem;
use sqlformat::{FormatOptions, QueryParams, format};

pub enum EditorEvent {
    ExecuteQuery(String),
}

impl EventEmitter<EditorEvent> for Editor {}

pub struct Editor {
    input_state: Entity<InputState>,
    _subscribes: Vec<Subscription>,
    lsp_store: LspStore,
    is_executing: bool,
    is_formatting: bool,
}

impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let default_language = "sql".to_string();
        let lsp_store = LspStore::new();

        let input_state = cx.new(|cx| {
            let mut i = InputState::new(window, cx)
                .code_editor(default_language)
                .line_number(true)
                .indent_guides(true)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .placeholder("Enter your SQL query here...");
            i.set_value("SELECT * FROM products;", window, cx);
            i.lsp.completion_provider = Some(Rc::new(lsp_store.clone()));
            i
        });

        let _subscribes = vec![
            // cx.observe_global::<ConnectionState>(move |this, cx| {
            //     let state = cx.global::<ConnectionState>();
            //     this.active_connection = state.active_connection.clone();
            //     cx.notify();
            // }),
            cx.observe_global::<EditorState>(move |this, cx| {
                let tables = cx.global::<EditorState>().tables.clone();
                let completions = tables
                    .iter()
                    .map(|table| {
                        let table = table.clone();
                        CompletionItem {
                            label: table.table_name.into(),
                            kind: Some(lsp_types::CompletionItemKind::KEYWORD),
                            detail: Some(
                                format!("{}:{}", table.table_schema, table.table_type).into(),
                            ),
                            ..Default::default()
                        }
                    })
                    .collect::<Vec<_>>();
                this.lsp_store.add_schema_completions(completions);
                cx.notify();
            }),
        ];

        Self {
            input_state,
            lsp_store,
            is_executing: false,
            is_formatting: false,
            _subscribes,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn format_query(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.is_formatting = true;
        cx.notify();

        let sql = self.input_state.read(cx).value().clone();
        let query = sql.trim();
        let formatted = format(query, &QueryParams::None, &FormatOptions::default());
        self.input_state.update(cx, |input_state, cx| {
            input_state.set_value(formatted, window, cx);
            self.is_formatting = false;
            cx.notify();
        })
    }

    pub fn execute_query(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let query = self.input_state.read(cx).value().clone();
        if !query.trim().is_empty() {
            cx.emit(EditorEvent::ExecuteQuery(query.to_string()));
        }
    }

    pub fn set_executing(&mut self, executing: bool, cx: &mut Context<Self>) {
        self.is_executing = executing;
        cx.notify();
    }
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let execute_button = Button::new("execute-query")
            .tooltip(if self.is_executing {
                "Executing..."
            } else {
                "Execute"
            })
            .icon(Icon::empty().path("icons/play.svg"))
            .small()
            .primary()
            .ghost()
            .disabled(self.is_executing)
            .on_click(cx.listener(Self::execute_query));

        let format_button = Button::new("execute-format")
            .tooltip(if self.is_formatting {
                "Formatting..."
            } else {
                "Format"
            })
            .icon(Icon::empty().path("icons/brush-cleaning.svg"))
            .small()
            .primary()
            .ghost()
            .disabled(self.is_formatting)
            .on_click(cx.listener(Self::format_query));

        let toolbar = h_flex()
            .id("editor-toolbar")
            .justify_end()
            .items_center()
            .pb_2()
            .px_2()
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(format_button)
                    .child(execute_button),
            );

        v_flex()
            .size_full()
            .child(
                div()
                    .id("editor-content")
                    .bg(cx.theme().background)
                    .w_full()
                    .flex_1()
                    .p_2()
                    .font_family("Monaco")
                    .text_size(px(12.))
                    .child(Input::new(&self.input_state).h_full()),
            )
            .child(toolbar)
    }
}
