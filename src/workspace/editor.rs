use std::rc::Rc;

use crate::{
    services::{ConnectionInfo, SqlCompletionProvider},
    state::{ConnectionState, DatabaseState, EditorState, change_database, disconnect},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    divider::Divider,
    h_flex,
    input::{Input, InputState, TabSize},
    select::{Select, SelectEvent, SelectState},
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
    _subscriptions: Vec<Subscription>,
    lsp_store: SqlCompletionProvider,
    is_executing: bool,
    is_formatting: bool,
    active_connection: Option<ConnectionInfo>,
    db_select: Entity<SelectState<Vec<SharedString>>>,
}

impl Editor {
    pub fn set_query(&mut self, query: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        cx.update_entity(&self.input_state, |i, cx| {
            i.set_value(query, window, cx);
            cx.notify();
        });
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let default_language = "sql".to_string();
        let lsp_store = SqlCompletionProvider::new();

        let input_state = cx.new(|cx| {
            let mut i = InputState::new(window, cx)
                .code_editor(default_language)
                .line_number(true)
                .indent_guides(false)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .placeholder("Enter your SQL query here...");
            i.lsp.completion_provider = Some(Rc::new(lsp_store.clone()));
            i
        });

        let db_select = cx.new(|cx| SelectState::new(Vec::<SharedString>::new(), None, window, cx));

        let _subscriptions = vec![
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
            cx.observe_global_in::<ConnectionState>(window, move |this, win, cx| {
                let state = cx.global::<ConnectionState>();
                let active_connection = state.active_connection.clone();

                this.active_connection = active_connection.clone();

                if let Some(conn) = active_connection.clone() {
                    cx.update_entity(&this.db_select.clone(), |select, cx| {
                        select.set_selected_value(&conn.database.clone().into(), win, cx);
                    });
                }

                cx.notify();
            }),
            cx.observe_global_in::<DatabaseState>(window, move |this, win, cx| {
                let state = cx.global::<DatabaseState>();
                let databases = state.databases.clone();

                let databases: Vec<SharedString> = databases
                    .iter()
                    .map(|db| db.datname.clone().into())
                    .collect();

                cx.update_entity(&this.db_select.clone(), |select, cx| {
                    select.set_items(databases, win, cx);
                });

                cx.notify();
            }),
        ];

        cx.subscribe_in(&db_select, window, Self::on_select_database_event)
            .detach();

        Self {
            input_state,
            lsp_store,
            is_executing: false,
            is_formatting: false,
            active_connection: None,
            db_select,
            _subscriptions,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn on_select_database_event(
        &mut self,
        _: &Entity<SelectState<Vec<SharedString>>>,
        event: &SelectEvent<Vec<SharedString>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SelectEvent::Confirm(value) => {
                if let Some(database) = value {
                    change_database(database.to_string(), cx)
                }
            }
        }
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
        let connection_name = self.active_connection.clone().map(|x| x.name.clone());

        let disconnect_button = Button::new("disconnect_button")
            .icon(Icon::empty().path("icons/power.svg"))
            .small()
            .danger()
            .ghost()
            .tooltip("Disconnect")
            .on_click(|_evt, _win, cx| disconnect(cx));

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
            .icon(Icon::empty().path("icons/align-start-vertical.svg"))
            .small()
            .primary()
            .ghost()
            .disabled(self.is_formatting)
            .on_click(cx.listener(Self::format_query));

        let toolbar = h_flex()
            .id("editor-toolbar")
            .justify_between()
            .items_center()
            .p_2()
            .when(connection_name.is_some(), |el| {
                el.child(
                    h_flex()
                        .pl_2()
                        .gap_0()
                        .items_center()
                        .text_color(cx.theme().accent_foreground)
                        .child(Icon::empty().path("icons/database.svg"))
                        .child(
                            Select::new(&self.db_select.clone())
                                .appearance(false)
                                .menu_width(px(200.)), // Keep menu width for longer db names
                        ),
                )
            })
            .when(connection_name.is_none(), |el| el.child(div()))
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(format_button)
                    .child(execute_button)
                    .child(Divider::vertical())
                    .child(disconnect_button),
            );

        v_flex().size_full().child(toolbar).child(
            div()
                .id("editor-content")
                .bg(cx.theme().background)
                .w_full()
                .flex_1()
                .px_2()
                .pb_2()
                .font_family("Monaco")
                .text_size(px(12.))
                .child(Input::new(&self.input_state).h_full()),
        )
    }
}
