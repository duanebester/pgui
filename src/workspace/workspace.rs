use super::connections::ConnectionManager;
use super::editor::Editor;
use super::editor::EditorEvent;
use super::footer_bar::{FooterBar, FooterBarEvent};
use super::header_bar::HeaderBar;
use super::tables_tree::{TableEvent, TablesTree};

use crate::services::{EnhancedQueryExecutionResult, TableInfo};
use crate::state::{ConnectionState, ConnectionStatus};
use crate::workspace::agent_panel::AgentPanel;
use crate::workspace::results_panel::EnhancedResultsPanel;
use gpui::prelude::FluentBuilder as _;
use gpui::*;

use gpui_component::ActiveTheme;
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::spinner::Spinner;

pub struct Workspace {
    connection_state: ConnectionStatus,
    header_bar: Entity<HeaderBar>,
    footer_bar: Entity<FooterBar>,
    tables_tree: Entity<TablesTree>,
    editor: Entity<Editor>,
    agent_panel: Entity<AgentPanel>,
    connection_manager: Entity<ConnectionManager>,
    enhanced_results: Entity<EnhancedResultsPanel>,
    _subscriptions: Vec<Subscription>,
    show_tables: bool,
    show_agent: bool,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let header_bar = HeaderBar::view(window, cx);
        let footer_bar = FooterBar::view(window, cx);
        let tables_tree = TablesTree::view(window, cx);
        let agent_panel = AgentPanel::view(window, cx);
        let editor = Editor::view(window, cx);
        let enhanced_results = EnhancedResultsPanel::view(window, cx);
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
                    FooterBarEvent::HideTables => {
                        this.show_tables = false;
                    }
                    FooterBarEvent::ShowTables => {
                        this.show_tables = true;
                    }
                    FooterBarEvent::HideAgent => {
                        this.show_agent = false;
                    }
                    FooterBarEvent::ShowAgent => {
                        this.show_agent = true;
                    }
                }
                cx.notify();
            }),
        ];

        Self {
            header_bar,
            footer_bar,
            connection_manager,
            tables_tree,
            editor,
            agent_panel,
            enhanced_results,
            _subscriptions,
            connection_state: ConnectionStatus::Disconnected,
            show_tables: true,
            show_agent: false,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn execute_query(&mut self, query: String, cx: &mut Context<Self>) {
        // Set editor to executing state
        self.editor.update(cx, |editor, cx| {
            editor.set_executing(true, cx);
        });

        // Get database manager from global state
        let db_manager = cx.global::<ConnectionState>().db_manager.clone();

        cx.spawn(async move |this, cx| {
            let result = db_manager.execute_query_enhanced(&query).await;
            this.update(cx, |this, cx| {
                // Update results panel
                this.enhanced_results.update(cx, |results, cx| {
                    results.update_result(result, cx);
                });

                // Set editor back to normal state
                this.editor.update(cx, |editor, cx| {
                    editor.set_executing(false, cx);
                });

                cx.notify();
            })
            .ok();
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
                        this.enhanced_results.update(cx, |results, cx| {
                            results.update_result(query_result, cx);
                        });
                    }
                    Err(e) => {
                        this.enhanced_results.update(cx, |results, cx| {
                            results.update_result(
                                EnhancedQueryExecutionResult::Error(format!(
                                    "Failed to load table columns: {}",
                                    e
                                )),
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
                            .child(self.enhanced_results.clone()),
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
            .when(self.show_agent.clone(), |d| d.child(agent));

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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
    }
}
