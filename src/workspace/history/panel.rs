use chrono::{DateTime, Utc};
use gpui::{
    AnyElement, App, AppContext, ClickEvent, Context, Entity, EventEmitter,
    InteractiveElement as _, IntoElement, ListAlignment, ListState, ParentElement, Render,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div, list,
    prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    v_flex,
};

use crate::{
    services::{AppStore, ConnectionInfo, storage::QueryHistoryEntry},
    state::ConnectionState,
};

/// Event emitted when a history entry is selected
pub enum HistoryEvent {
    /// User wants to load this SQL into the editor
    LoadQuery(String),
}

impl EventEmitter<HistoryEvent> for HistoryPanel {}

pub struct HistoryPanel {
    list_state: ListState,
    history_entries: Vec<QueryHistoryEntry>,
    filtered_entries: Vec<QueryHistoryEntry>,
    active_connection: Option<ConnectionInfo>,
    is_loading: bool,
    _subscriptions: Vec<Subscription>,
}

#[allow(dead_code)]
impl HistoryPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let list_state = ListState::new(0, ListAlignment::Top, px(20.));

        let _subscriptions = vec![cx.observe_global::<ConnectionState>(move |this, cx| {
            let state = cx.global::<ConnectionState>();
            let new_connection = state.active_connection.clone();

            // Only reload if connection changed
            if this.active_connection.as_ref().map(|c| &c.id)
                != new_connection.as_ref().map(|c| &c.id)
            {
                this.active_connection = new_connection;
                this.load_history(cx);
            }
            cx.notify();
        })];

        Self {
            list_state,
            history_entries: Vec::new(),
            filtered_entries: Vec::new(),
            active_connection: None,
            is_loading: false,
            _subscriptions,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn filter_entries(&mut self, search_text: &str) {
        if search_text.is_empty() {
            self.filtered_entries = self.history_entries.clone();
        } else {
            self.filtered_entries = self
                .history_entries
                .iter()
                .filter(|entry| entry.sql.to_lowercase().contains(search_text))
                .cloned()
                .collect();
        }
    }

    fn load_history(&mut self, cx: &mut Context<Self>) {
        let Some(connection) = self.active_connection.clone() else {
            self.history_entries.clear();
            self.filtered_entries.clear();
            self.list_state = ListState::new(0, ListAlignment::Top, px(20.));
            cx.notify();
            return;
        };

        self.is_loading = true;
        cx.notify();

        let connection_id = connection.id;

        cx.spawn(async move |this, cx| {
            let result = async {
                let store = AppStore::singleton().await?;
                store
                    .history()
                    .load_for_connection(&connection_id, 100)
                    .await
            }
            .await;

            this.update(cx, |this, cx| {
                this.is_loading = false;
                match result {
                    Ok(entries) => {
                        this.history_entries = entries;
                        this.filter_entries("");
                        this.list_state = ListState::new(
                            this.filtered_entries.len(),
                            ListAlignment::Top,
                            px(20.),
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to load history: {}", e);
                        this.history_entries.clear();
                        this.filtered_entries.clear();
                        this.list_state = ListState::new(0, ListAlignment::Top, px(20.));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Reload history - can be called after executing a query
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.load_history(cx);
    }

    fn on_refresh(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.load_history(cx);
    }

    fn on_clear_history(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(connection) = self.active_connection.clone() else {
            return;
        };

        let connection_id = connection.id;

        cx.spawn(async move |this, cx| {
            let result = async {
                let store = AppStore::singleton().await?;
                store.history().clear_for_connection(&connection_id).await
            }
            .await;

            this.update(cx, |this, cx| {
                if result.is_ok() {
                    this.history_entries.clear();
                    this.filtered_entries.clear();
                    this.list_state = ListState::new(0, ListAlignment::Top, px(20.));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn on_entry_click(&mut self, sql: String, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(HistoryEvent::LoadQuery(sql));
    }

    fn format_relative_time(executed_at: DateTime<Utc>) -> String {
        let now = Utc::now();
        let duration = now.signed_duration_since(executed_at);

        if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            let mins = duration.num_minutes();
            format!("{} min{} ago", mins, if mins == 1 { "" } else { "s" })
        } else if duration.num_hours() < 24 {
            let hours = duration.num_hours();
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else if duration.num_days() < 7 {
            let days = duration.num_days();
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        } else {
            executed_at.format("%b %d, %Y").to_string()
        }
    }

    fn truncate_sql(sql: &str, max_len: usize) -> String {
        // Collapse whitespace (including newlines) into single spaces to show more of the query
        let collapsed: String = sql.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.len() > max_len {
            format!("{}...", &collapsed[..max_len])
        } else {
            collapsed
        }
    }

    fn render_entry(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(entry) = self.filtered_entries.get(ix).cloned() else {
            return div().into_any_element();
        };

        let sql = entry.sql.clone();
        let truncated_sql = Self::truncate_sql(&sql, 40);
        let relative_time = Self::format_relative_time(entry.executed_at);

        let execution_info = if let Some(rows) = entry.rows_affected {
            format!("{}ms • {} rows", entry.execution_time_ms, rows)
        } else {
            format!("{}ms", entry.execution_time_ms)
        };

        let status_icon = if entry.success {
            Icon::new(IconName::CircleCheck).text_color(cx.theme().success)
        } else {
            Icon::new(IconName::CircleX).text_color(cx.theme().danger)
        };

        let bg_color = if ix % 2 == 0 {
            cx.theme().list
        } else {
            cx.theme().list_even
        };

        div()
            .p_1()
            .child(
                div()
                    .id(("history-entry", ix))
                    .w_full()
                    .p_2()
                    .bg(bg_color)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(cx.theme().list_active)
                            .border_color(cx.theme().list_active_border)
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.on_entry_click(sql.clone(), window, cx);
                    }))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(status_icon.size_4())
                                    .child(
                                        Label::new(truncated_sql)
                                            .text_sm()
                                            .font_medium()
                                            .line_height(px(18.)),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .pl(px(24.))
                                    .child(
                                        Label::new(execution_info)
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(
                                        Label::new("•")
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(
                                        Label::new(relative_time)
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            )
                            .when(!entry.success && entry.error_message.is_some(), |el| {
                                el.child(
                                    h_flex().pl(px(24.)).child(
                                        Label::new(
                                            entry
                                                .error_message
                                                .clone()
                                                .unwrap_or_default()
                                                .chars()
                                                .take(50)
                                                .collect::<String>(),
                                        )
                                        .text_xs()
                                        .text_color(cx.theme().danger),
                                    ),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
}

impl Render for HistoryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_connection = self.active_connection.is_some();
        let entry_count = self.filtered_entries.len();

        let refresh_button = Button::new("refresh-history")
            .icon(Icon::empty().path("icons/rotate-ccw.svg"))
            .small()
            .ghost()
            .tooltip("Refresh History")
            .disabled(!has_connection || self.is_loading)
            .on_click(cx.listener(Self::on_refresh));

        let clear_button = Button::new("clear-history")
            .icon(Icon::empty().path("icons/trash.svg"))
            .small()
            .ghost()
            .tooltip("Clear History")
            .disabled(!has_connection || self.history_entries.is_empty())
            .on_click(cx.listener(Self::on_clear_history));

        let header = h_flex()
            .justify_between()
            .items_center()
            .child(Label::new("History").font_bold().text_base())
            .child(h_flex().gap_1().child(refresh_button).child(clear_button));

        let content = if !has_connection {
            div().flex_1().flex().items_center().justify_center().child(
                Label::new("Connect to a database to see history")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
        } else if self.is_loading {
            div().flex_1().flex().items_center().justify_center().child(
                Label::new("Loading...")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
        } else if entry_count == 0 {
            div().flex_1().flex().items_center().justify_center().child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::empty()
                            .path("icons/archive.svg")
                            .size_8()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(if self.history_entries.is_empty() {
                            "No queries yet"
                        } else {
                            "No matching queries"
                        })
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                    ),
            )
        } else {
            div().flex_1().overflow_hidden().child(
                list(
                    self.list_state.clone(),
                    cx.processor(|this, ix, window, cx| this.render_entry(ix, window, cx)),
                )
                .size_full(),
            )
        };

        v_flex()
            .size_full()
            .gap_2()
            .p_2()
            .child(header)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} {}",
                        entry_count,
                        if entry_count == 1 { "query" } else { "queries" }
                    )),
            )
            .child(content)
    }
}
