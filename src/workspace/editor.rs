use std::time::Duration;

use crate::{
    services::{ConnectionInfo, SqlQueryAnalyzer},
    state::{ConnectionState, EditorState},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputEvent, InputState, TabSize, TextInput},
    v_flex,
};
use sqlformat::{FormatOptions, QueryParams, format};

pub enum EditorEvent {
    ExecuteQuery(String),
}

impl EventEmitter<EditorEvent> for Editor {}

pub struct Editor {
    active_connection: Option<ConnectionInfo>,
    input_state: Entity<InputState>,
    _subscribes: Vec<Subscription>,
    sql_analyzer: SqlQueryAnalyzer,
    is_executing: bool,
    is_formatting: bool,
    debounce_task: Option<Task<()>>,
    debounce_duration: Duration,
}

impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let default_language = "sql".to_string();
        let input_state = cx.new(|cx| {
            let mut i = InputState::new(window, cx)
                .code_editor(default_language)
                .line_number(true)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .placeholder("Enter your SQL query here...");
            i.set_value("SELECT * FROM products;", window, cx);
            i
        });

        let sql_analyzer = SqlQueryAnalyzer::new();

        let _subscribes = vec![
            cx.observe_global::<ConnectionState>(move |this, cx| {
                let state = cx.global::<ConnectionState>();
                this.active_connection = state.active_connection.clone();
                cx.notify();
            }),
            cx.subscribe(&input_state, |_, _, _: &InputEvent, cx| {
                cx.notify();
            }),
            cx.subscribe_in(
                &input_state,
                window,
                |this, input_state, _: &InputEvent, window, cx| {
                    let input_state = input_state.clone();
                    let duration = this.debounce_duration;
                    // Dropping the old task automatically cancels it
                    this.debounce_task = Some(cx.spawn_in(window, async move |editor, cx| {
                        Timer::after(duration).await;

                        _ = editor.update_in(cx, move |this, _, cx| {
                            let text = input_state.read(cx).value().clone();
                            let queries = this.sql_analyzer.detect_queries(&text);
                            println!("Queries: {:?}", queries);

                            cx.update_global::<EditorState, _>(|state, _cx| {
                                state.sql_queries = queries;
                            });
                        });
                    }));
                },
            ),
        ];

        Self {
            input_state,
            sql_analyzer,
            active_connection: None,
            is_executing: false,
            is_formatting: false,
            debounce_task: None,
            debounce_duration: Duration::from_millis(250),
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

        let connection_name = self.active_connection.clone().map(|x| x.name.clone());
        let disconnect_button = Button::new("disconnect_button")
            .icon(Icon::empty().path("icons/plug-zap.svg"))
            .small()
            .danger()
            .ghost()
            .tooltip("Disconnect")
            .on_click(|_evt, _win, cx| ConnectionState::disconnect(cx));

        let toolbar = h_flex()
            .id("editor-toolbar")
            .bg(cx.theme().table_head)
            .border_b_1()
            .border_color(cx.theme().border)
            .justify_between()
            .items_center()
            .py_1()
            .px_2()
            .when(connection_name.is_some(), |d| {
                d.child(
                    div()
                        .flex()
                        .justify_center()
                        .items_center()
                        .gap_1()
                        .text_color(cx.theme().accent_foreground)
                        .text_xs()
                        .child(format!("Connected to {}", connection_name.unwrap()))
                        .child(disconnect_button),
                )
            })
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(format_button)
                    .child(execute_button),
            );

        v_flex().size_full().child(toolbar).child(
            div()
                .id("editor-content")
                .bg(cx.theme().background)
                .w_full()
                .flex_1()
                .p_2()
                .font_family("Monaco")
                .text_size(px(12.))
                .child(TextInput::new(&self.input_state).h_full()),
        )
    }
}
