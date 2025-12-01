use super::connections::ConnectionManager;
use super::editor::Editor;
use super::editor::EditorEvent;
use super::footer_bar::{FooterBar, FooterBarEvent};
use super::header_bar::HeaderBar;
use super::tables::{TableEvent, TablesTree};

use crate::services::AppStore;
use crate::services::{ErrorResult, QueryExecutionResult, TableInfo};
use crate::state::{ConnectionState, ConnectionStatus};
use crate::workspace::agent::AgentPanel;
use crate::workspace::history::HistoryEvent;
use crate::workspace::history::HistoryPanel;
use crate::workspace::results::ResultsPanel;
use gpui::prelude::FluentBuilder as _;
use gpui::*;

use gpui_component::ActiveTheme;
use gpui_component::Root;
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::spinner::Spinner;

pub struct Workspace {
    connection_state: ConnectionStatus,
    header_bar: Entity<HeaderBar>,
    footer_bar: Entity<FooterBar>,
    tables_tree: Entity<TablesTree>,
    editor: Entity<Editor>,
    agent_panel: Entity<AgentPanel>,
    history_panel: Entity<HistoryPanel>,
    connection_manager: Entity<ConnectionManager>,
    results_panel: Entity<ResultsPanel>,
    _subscriptions: Vec<Subscription>,
    show_tables: bool,
    show_agent: bool,
    show_history: bool,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let header_bar = HeaderBar::view(window, cx);
        let footer_bar = FooterBar::view(window, cx);
        let tables_tree = TablesTree::view(window, cx);
        let agent_panel = AgentPanel::view(window, cx);
        let history_panel = HistoryPanel::view(window, cx);
        let editor = Editor::view(window, cx);
        let results_panel = ResultsPanel::view(window, cx);
        let connection_manager = ConnectionManager::view(window, cx);

        let _subscriptions = vec![
            cx.observe_global::<ConnectionState>(move |this, cx| {
                this.connection_state = cx.global::<ConnectionState>().connection_state.clone();
                cx.notify();
            }),
            cx.subscribe(&editor, |this, _, event: &EditorEvent, cx| match event {
                EditorEvent::ExecuteQuery(query) => {
                    this.execute_query(query.clone(), cx);
                }
            }),
            cx.subscribe(&tables_tree, |this, _, event: &TableEvent, cx| {
                this.handle_table_event(event, cx);
            }),
            cx.subscribe(&footer_bar, |this, _, event: &FooterBarEvent, cx| {
                match event {
                    FooterBarEvent::ToggleTables(show) => {
                        this.show_tables = *show;
                    }
                    FooterBarEvent::ToggleAgent(show) => {
                        this.show_agent = *show;
                    }
                    FooterBarEvent::ToggleHistory(show) => {
                        this.show_history = *show;
                    }
                }
                cx.notify();
            }),
            // Subscribe to history panel events
            cx.subscribe_in(
                &history_panel,
                window,
                |this, _, event: &HistoryEvent, win, cx| match event {
                    HistoryEvent::LoadQuery(sql) => {
                        this.load_query_into_editor(sql.clone(), win, cx);
                    }
                },
            ),
        ];

        Self {
            header_bar,
            footer_bar,
            connection_manager,
            tables_tree,
            editor,
            agent_panel,
            history_panel,
            results_panel,
            _subscriptions,
            connection_state: ConnectionStatus::Disconnected,
            show_tables: true,
            show_agent: false,
            show_history: false,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn load_query_into_editor(&mut self, sql: String, window: &mut Window, cx: &mut App) {
        self.editor.update(cx, |editor, cx| {
            editor.set_query(sql, window, cx);
        });
    }

    fn execute_query(&mut self, query: String, cx: &mut Context<Self>) {
        // Set editor to executing state
        self.editor.update(cx, |editor, cx| {
            editor.set_executing(true, cx);
        });

        // Get database manager from global state
        let db_manager = cx.global::<ConnectionState>().db_manager.clone();
        let active_connection = cx.global::<ConnectionState>().active_connection.clone();

        cx.spawn(async move |this, cx| {
            let result = db_manager.execute_query_enhanced(&query).await;
            // Extract execution info before moving result
            let (execution_time_ms, rows_affected) = match &result {
                QueryExecutionResult::Modified(modified) => (
                    Some(modified.execution_time_ms as i64),
                    Some(modified.rows_affected as i64),
                ),
                QueryExecutionResult::Select(r) => (Some(r.execution_time_ms as i64), None),
                QueryExecutionResult::Error(err) => (Some(err.execution_time_ms as i64), None),
            };

            this.update(cx, |this, cx| {
                // Update results panel
                this.results_panel.update(cx, |results, cx| {
                    results.update_result(result, cx);
                });

                // Set editor back to normal state
                this.editor.update(cx, |editor, cx| {
                    editor.set_executing(false, cx);
                });

                cx.notify();
            })
            .ok();

            if let Some(conn) = active_connection {
                if let Ok(store) = AppStore::singleton().await {
                    let _ = store
                        .history()
                        .record(
                            &conn.id,
                            &query.clone(),
                            execution_time_ms.unwrap_or(0),
                            rows_affected,
                            true,
                            None,
                        )
                        .await;
                }
            }
        })
        .detach();
    }

    fn handle_table_event(&mut self, event: &TableEvent, cx: &mut Context<Self>) {
        match event {
            TableEvent::TableSelected(table) => {
                self.show_table_columns(table.clone(), cx);
            }
        }
    }

    fn show_table_columns(&mut self, table: TableInfo, cx: &mut Context<Self>) {
        // Get database manager from global state
        let db_manager = cx.global::<ConnectionState>().db_manager.clone();

        cx.spawn(async move |this, cx| {
            let result = db_manager
                .get_table_columns(&table.table_name, &table.table_schema)
                .await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(query_result) => {
                        this.results_panel.update(cx, |results, cx| {
                            results.update_result(query_result, cx);
                        });
                    }
                    Err(e) => {
                        this.results_panel.update(cx, |results, cx| {
                            results.update_result(
                                QueryExecutionResult::Error(ErrorResult {
                                    execution_time_ms: 0,
                                    message: format!("Failed to load table columns: {}", e),
                                }),
                                cx,
                            );
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn render_disconnected(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let content = div()
            .id("connection-manager")
            .flex()
            .flex_1()
            .bg(cx.theme().background)
            .child(self.connection_manager.clone());

        content
    }

    fn render_connected(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let sidebar = div()
            .id("connected-sidebar")
            .flex()
            .flex_col()
            .h_full()
            .border_color(cx.theme().border)
            .border_r_1()
            .min_w(px(300.0))
            .child(self.tables_tree.clone());

        let agent = div()
            .id("connected-agent")
            .flex()
            .flex_col()
            .h_full()
            .w(px(400.))
            .border_color(cx.theme().border)
            .border_l_1()
            .child(self.agent_panel.clone());

        let history = div()
            .id("connected-history")
            .flex()
            .flex_col()
            .h_full()
            .w(px(400.))
            .border_color(cx.theme().border)
            .border_l_1()
            .child(self.history_panel.clone());

        let main = div()
            .id("connected-main")
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .w_full()
            .overflow_hidden()
            .child(
                v_resizable("resizable-results")
                    .child(
                        resizable_panel()
                            .size(px(400.))
                            .size_range(px(200.)..px(800.))
                            .child(self.editor.clone()),
                    )
                    .child(
                        resizable_panel()
                            .size(px(200.))
                            .child(self.results_panel.clone()),
                    ),
            );

        let content = div()
            .id("connected-content")
            .flex()
            .flex_row()
            .flex_1()
            .h_full()
            .bg(cx.theme().background)
            .when(self.show_tables.clone(), |d| d.child(sidebar))
            .child(main)
            .when(self.show_agent.clone(), |d| d.child(agent))
            .when(self.show_history.clone(), |d| d.child(history));

        content
    }

    fn render_loading(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let content = div()
            .id("loading-content")
            .flex()
            .flex_grow()
            .bg(cx.theme().background)
            .justify_center()
            .items_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(Spinner::new())
                    .child("Loading"),
            );

        content
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.connection_state.clone() {
            ConnectionStatus::Disconnected => self.render_disconnected(cx),
            ConnectionStatus::Connected => self.render_connected(cx),
            ConnectionStatus::Disconnecting => self.render_loading(cx),
            ConnectionStatus::Connecting => self.render_loading(cx),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.header_bar.clone())
            .child(content)
            .child(self.footer_bar.clone())
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
