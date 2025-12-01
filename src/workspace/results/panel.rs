use crate::{services::QueryExecutionResult, workspace::results::EnhancedResultsTableDelegate};
use gpui::*;
use gpui_component::{
    ActiveTheme as _, h_flex,
    label::Label,
    table::{Table, TableState},
    v_flex,
};

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
}

impl Render for ResultsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.current_result {
            Some(QueryExecutionResult::Select(_result)) => v_flex()
                .size_full()
                .p_4()
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
