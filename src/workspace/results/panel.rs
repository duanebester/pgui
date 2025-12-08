use crate::{
    services::{
        QueryExecutionResult,
        export::{stream_to_csv, stream_to_ndjson},
        export_to_csv, export_to_json,
    },
    state::ConnectionState,
    workspace::results::EnhancedResultsTableDelegate,
};
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Icon, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    notification::NotificationType,
    table::{Table, TableState},
    v_flex,
};

pub enum ExportFormat {
    Csv,
    Json,
}

pub struct ResultsPanel {
    current_result: Option<QueryExecutionResult>,
    table: Entity<TableState<EnhancedResultsTableDelegate>>,
}

impl ResultsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = EnhancedResultsTableDelegate::new();
        let table = cx.new(|cx| TableState::new(delegate, window, cx).sortable(false));

        Self {
            current_result: None,
            table,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn update_result(&mut self, result: QueryExecutionResult, cx: &mut Context<Self>) {
        self.current_result = Some(result.clone());
        if let QueryExecutionResult::Select(x) = result {
            self.table.update(cx, |table, cx| {
                table.delegate_mut().update(x.clone());
                table.refresh(cx);
            });
        }
        cx.notify();
    }

    fn stream_export_results(
        &mut self,
        format: ExportFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(QueryExecutionResult::Select(result)) = &self.current_result else {
            return;
        };

        let sql = result.original_query.clone();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let suggested_name = match format {
            ExportFormat::Csv => format!("export_{}.csv", timestamp),
            ExportFormat::Json => format!("export_{}.ndjson", timestamp),
        };

        let home = dirs::home_dir().unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&home, Some(&suggested_name));

        cx.spawn_in(window, async move |_this, cx| {
            if let Ok(Ok(Some(path))) = receiver.await {
                let db_manager_result =
                    cx.read_global::<ConnectionState, _>(|state, _, _| state.db_manager.clone());

                let result: anyhow::Result<u64> = if let Ok(db_manager) = db_manager_result {
                    cx.background_executor()
                        .spawn(async move {
                            let stream = db_manager
                                .stream_query(&sql)
                                .await
                                .map_err(|e| anyhow::anyhow!(e))?;

                            match format {
                                ExportFormat::Csv => stream_to_csv(stream, &path).await,
                                ExportFormat::Json => stream_to_ndjson(stream, &path).await,
                            }
                        })
                        .await
                } else {
                    Ok(0)
                };

                match result {
                    Ok(count) => {
                        let _ = cx.update(|window, cx| {
                            let info: SharedString = format!("Exported {} rows", count).into();
                            window.push_notification((NotificationType::Info, info), cx);
                        });
                    }
                    Err(e) => {
                        tracing::error!("Stream export failed: {}", e);
                        let _ = cx.update(|window, cx| {
                            window
                                .push_notification((NotificationType::Error, "Export failed"), cx);
                        });
                    }
                }
            }
        })
        .detach();
    }

    #[allow(dead_code)]
    fn export_results(
        &mut self,
        format: ExportFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(QueryExecutionResult::Select(result)) = &self.current_result else {
            return;
        };

        let result = result.clone();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");

        let (_extension, suggested_name) = match format {
            ExportFormat::Csv => ("csv", format!("export_{}.csv", timestamp)),
            ExportFormat::Json => ("json", format!("export_{}.json", timestamp)),
        };

        // Use GPUI's native file dialog
        let home = dirs::home_dir().unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&home, Some(&suggested_name));

        cx.spawn_in(window, async move |_this, cx| {
            if let Ok(Ok(Some(path))) = receiver.await {
                let result: anyhow::Result<()> = async {
                    let content = match format {
                        ExportFormat::Csv => export_to_csv(&result)?,
                        ExportFormat::Json => export_to_json(&result)?,
                    };
                    async_fs::write(&path, content).await?;
                    Ok(())
                }
                .await;

                if let Err(e) = result {
                    tracing::error!("Export failed: {}", e);
                    let _ = cx.update(|window, cx| {
                        window.push_notification(
                            (
                                NotificationType::Error,
                                "Failed to save file. Please try again.",
                            ),
                            cx,
                        );
                    });
                } else {
                    let _ = cx.update(|window, cx| {
                        window.push_notification(
                            (NotificationType::Info, "File saved successfully."),
                            cx,
                        );
                    });
                }
            }
        })
        .detach();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_1()
            .justify_end()
            .items_center()
            .child(
                Button::new("export-csv")
                    .icon(Icon::empty().path("icons/file-spreadsheet.svg"))
                    .small()
                    .ghost()
                    .tooltip("Export CSV")
                    .on_click(cx.listener(|this, _, win, cx| {
                        this.stream_export_results(ExportFormat::Csv, win, cx);
                    })),
            )
            .child(
                Button::new("export-json")
                    .icon(Icon::empty().path("icons/file-braces.svg"))
                    .small()
                    .ghost()
                    .tooltip("Export JSON")
                    .on_click(cx.listener(|this, _, win, cx| {
                        this.stream_export_results(ExportFormat::Json, win, cx);
                    })),
            )
    }
}

impl Render for ResultsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.current_result {
            Some(QueryExecutionResult::Select(_result)) => v_flex()
                .size_full()
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.render_toolbar(cx))
                .child(Table::new(&self.table.clone()).stripe(true)),
            Some(QueryExecutionResult::Modified(modified)) => {
                h_flex().size_full().items_center().justify_center().child(
                    Label::new(format!(
                        "Query executed successfully. {} rows affected in {}ms",
                        modified.rows_affected, modified.execution_time_ms
                    ))
                    .text_sm()
                    .text_color(cx.theme().accent_foreground),
                )
            }
            Some(QueryExecutionResult::Error(error)) => v_flex().size_full().p_4().child(
                div()
                    .p_4()
                    .bg(cx.theme().danger)
                    .border_1()
                    .border_color(cx.theme().danger)
                    .rounded(cx.theme().radius)
                    .child(
                        Label::new(format!("Error: {}", error.message))
                            .text_sm()
                            .text_color(cx.theme().danger_foreground),
                    ),
            ),
            _ => h_flex().size_full().items_center().justify_center().child(
                Label::new("Execute a query to see results here")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            ),
        }
    }
}
