use crate::connections_panel::{ConnectionEvent, ConnectionsPanel};
use crate::database::TableInfo;
use crate::editor::EditorEvent;
use crate::footer_bar::{FooterBar, FooterBarEvent};
use crate::header_bar::HeaderBar;
use crate::tables_panel::{TableEvent, TablesPanel};
use crate::{editor::Editor, results_panel::ResultsPanel};

use gpui::prelude::FluentBuilder;
use gpui::*;

use gpui_component::ActiveTheme;
use gpui_component::resizable::{ResizableState, resizable_panel, v_resizable};

pub struct Workspace {
    resize_state: Entity<ResizableState>,
    header_bar: Entity<HeaderBar>,
    footer_bar: Entity<FooterBar>,
    connections_panel: Entity<ConnectionsPanel>,
    tables_panel: Entity<TablesPanel>,
    editor: Entity<Editor>,
    results_panel: Entity<ResultsPanel>,
    _subscriptions: Vec<Subscription>,
    show_connections: bool,
    show_tables: bool,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let header_bar = HeaderBar::view(window, cx);
        let footer_bar = FooterBar::view(window, cx);
        let resize_state = ResizableState::new(cx);
        let connections_panel = ConnectionsPanel::view(window, cx);
        let tables_panel = TablesPanel::view(window, cx);
        let editor = Editor::view(window, cx);
        let results_panel = ResultsPanel::view(window, cx);

        let _subscriptions = vec![
            cx.subscribe(&editor, |this, _, event: &EditorEvent, cx| match event {
                EditorEvent::ExecuteQuery(query) => {
                    this.execute_query(query.clone(), cx);
                }
            }),
            cx.subscribe(
                &connections_panel,
                |this, _, event: &ConnectionEvent, cx| {
                    this.tables_panel.update(cx, |tables_panel, cx| {
                        tables_panel.handle_connection_event(event, cx);
                    });
                },
            ),
            cx.subscribe(&tables_panel, |this, _, event: &TableEvent, cx| {
                this.handle_table_event(event, cx);
            }),
            cx.subscribe(&footer_bar, |this, _, event: &FooterBarEvent, cx| {
                match event {
                    FooterBarEvent::ShowConnections => {
                        this.show_connections = true;
                        this.show_tables = false;
                    }
                    FooterBarEvent::ShowTables => {
                        this.show_connections = false;
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
            tables_panel,
            editor,
            results_panel,
            _subscriptions,
            show_connections: true,
            show_tables: false,
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

        // Get database manager from connections panel
        let db_manager = self.connections_panel.read(cx).db_manager.clone();

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
        // Get database manager from connections panel
        let db_manager = self.connections_panel.read(cx).db_manager.clone();

        cx.spawn(async move |this, cx| {
            let result = db_manager
                .get_table_columns(&table.table_name, &table.table_schema)
                .await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(query_result) => {
                        this.results_panel.update(cx, |results, cx| {
                            results.update_result(
                                crate::database::QueryExecutionResult::Select(query_result),
                                cx,
                            );
                        });
                    }
                    Err(e) => {
                        this.results_panel.update(cx, |results, cx| {
                            results.update_result(
                                crate::database::QueryExecutionResult::Error(format!(
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
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = div()
            .flex()
            .h_full()
            .border_color(cx.theme().border)
            .border_r_1()
            .min_w(px(300.0))
            .when(self.show_connections, |this| {
                this.child(self.connections_panel.clone())
            })
            .when(self.show_tables, |this| {
                this.child(self.tables_panel.clone())
            });

        let main = div().flex().flex_col().w_full().overflow_hidden().child(
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

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.header_bar.clone())
            .child(div().flex().flex_grow().child(sidebar).child(main))
            .child(self.footer_bar.clone())
    }
}
