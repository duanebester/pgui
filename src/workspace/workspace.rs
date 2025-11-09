use super::connections::ConnectionsPanel;
use super::editor::EditorEvent;
use super::footer_bar::{FooterBar, FooterBarEvent};
use super::header_bar::HeaderBar;
use super::tables_panel::{TableEvent, TablesPanel};
use super::{editor::Editor, results_panel::ResultsPanel};

use crate::services::{QueryExecutionResult, TableInfo};
use crate::state::{ConnectionState, ConnectionStatus};
use crate::workspace::connections::ConnectionForm;
use gpui::prelude::FluentBuilder as _;
use gpui::*;

use gpui_component::ActiveTheme;
use gpui_component::indicator::Indicator;
use gpui_component::resizable::{ResizableState, resizable_panel, v_resizable};

pub struct Workspace {
    connection_state: ConnectionStatus,
    resize_state: Entity<ResizableState>,
    header_bar: Entity<HeaderBar>,
    footer_bar: Entity<FooterBar>,
    connections_panel: Entity<ConnectionsPanel>,
    connection_form: Entity<ConnectionForm>,
    tables_panel: Entity<TablesPanel>,
    editor: Entity<Editor>,
    results_panel: Entity<ResultsPanel>,
    _subscriptions: Vec<Subscription>,
    show_tables: bool,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let header_bar = HeaderBar::view(window, cx);
        let footer_bar = FooterBar::view(window, cx);
        let resize_state = ResizableState::new(cx);
        let connections_panel = ConnectionsPanel::view(window, cx);
        let connection_form = ConnectionForm::view(window, cx);
        let tables_panel = TablesPanel::view(window, cx);
        let editor = Editor::view(window, cx);
        let results_panel = ResultsPanel::view(window, cx);

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
            cx.subscribe(&tables_panel, |this, _, event: &TableEvent, cx| {
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
                }
                cx.notify();
            }),
        ];

        Self {
            resize_state,
            header_bar,
            footer_bar,
            connections_panel,
            connection_form,
            tables_panel,
            editor,
            results_panel,
            _subscriptions,
            connection_state: ConnectionStatus::Disconnected,
            show_tables: true,
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
            let result = db_manager.execute_query(&query).await;

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
                            results.update_result(QueryExecutionResult::Select(query_result), cx);
                        });
                    }
                    Err(e) => {
                        this.results_panel.update(cx, |results, cx| {
                            results.update_result(
                                QueryExecutionResult::Error(format!(
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
        let sidebar = div()
            .id("disconnected-sidebar")
            .flex()
            .h_full()
            .border_color(cx.theme().border)
            .border_r_1()
            .min_w(px(300.0))
            .child(self.connections_panel.clone());

        let main = div()
            .id("disconnected-main")
            .flex()
            .flex_col()
            .w_full()
            .p_4()
            .child(self.connection_form.clone());

        let content = div()
            .id("disconnected-content")
            .flex()
            .flex_grow()
            .bg(cx.theme().background)
            .child(sidebar)
            .child(main);

        content
    }

    fn render_connected(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let sidebar = div()
            .id("connected-sidebar")
            .flex()
            .h_full()
            .border_color(cx.theme().border)
            .border_r_1()
            .min_w(px(300.0))
            .child(self.tables_panel.clone());

        let main = div()
            .id("connected-main")
            .flex()
            .flex_col()
            .w_full()
            .overflow_hidden()
            .child(
                v_resizable("resizable", self.resize_state.clone())
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
            .flex_grow()
            .bg(cx.theme().background)
            .when(self.show_tables.clone(), |d| d.child(sidebar))
            .child(main);

        content
    }

    fn render_connecting(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let content = div()
            .id("connecting-content")
            .flex()
            .flex_grow()
            .bg(cx.theme().background)
            .justify_center()
            .items_center()
            .child(Indicator::new());

        content
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.connection_state.clone() {
            ConnectionStatus::Disconnected => self.render_disconnected(cx),
            ConnectionStatus::Connected => self.render_connected(cx),
            ConnectionStatus::Connecting => self.render_connecting(cx),
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
