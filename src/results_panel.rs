use crate::database::{QueryExecutionResult, QueryResult};
use gpui::*;
use gpui_component::{ActiveTheme as _, StyledExt, h_flex, label::Label, v_flex};

pub struct ResultsPanel {
    current_result: Option<QueryExecutionResult>,
}

impl ResultsPanel {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            current_result: None,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn update_result(&mut self, result: QueryExecutionResult, cx: &mut Context<Self>) {
        self.current_result = Some(result);
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn clear_results(&mut self, cx: &mut Context<Self>) {
        self.current_result = None;
        cx.notify();
    }

    fn render_query_result(
        &self,
        result: &QueryResult,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if result.columns.is_empty() {
            return v_flex().p_4().child(
                Label::new(format!(
                    "Query completed successfully. {} rows returned in {}ms",
                    result.row_count, result.execution_time_ms
                ))
                .text_sm()
                .text_color(cx.theme().muted_foreground),
            );
        }

        let header = h_flex()
            .w_full()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().list_even)
            .children(result.columns.iter().map(|col| {
                div()
                    .flex_1()
                    .p_2()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(
                        Label::new(col.clone())
                            .font_bold()
                            .text_xs()
                            .whitespace_nowrap(),
                    )
            }));

        let rows_content = result.rows.iter().enumerate().map(|(row_idx, row)| {
            let bg_color = if row_idx % 2 == 0 {
                cx.theme().list
            } else {
                cx.theme().list_even
            };

            h_flex()
                .w_full()
                .bg(bg_color)
                .border_b_1()
                .border_color(cx.theme().border.opacity(0.3))
                .children(row.iter().map(|cell| {
                    div()
                        .flex_1()
                        .p_2()
                        .border_r_1()
                        .border_color(cx.theme().border.opacity(0.3))
                        .child(Label::new(cell.clone()).text_xs().whitespace_nowrap())
                }))
        });

        let footer = h_flex()
            .justify_between()
            .items_center()
            .p_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().list_even.opacity(0.5))
            .child(
                Label::new(format!("{} rows returned", result.row_count))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Label::new(format!("Executed in {}ms", result.execution_time_ms))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            );

        v_flex()
            .w_full()
            .h_full()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .overflow_hidden()
            .child(header)
            .child(div().flex_1().overflow_hidden().children(rows_content))
            .child(footer)
    }
}

impl Render for ResultsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.current_result {
            None => v_flex().size_full().items_center().justify_center().child(
                Label::new("Execute a query to see results here")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            ),
            Some(QueryExecutionResult::Select(result)) => v_flex()
                .size_full()
                .p_4()
                .child(self.render_query_result(result, cx)),
            Some(QueryExecutionResult::Modified {
                rows_affected,
                execution_time_ms,
            }) => v_flex().size_full().items_center().justify_center().child(
                Label::new(format!(
                    "Query executed successfully. {} rows affected in {}ms",
                    rows_affected, execution_time_ms
                ))
                .text_sm()
                .text_color(cx.theme().accent_foreground),
            ),
            Some(QueryExecutionResult::Error(error)) => v_flex().size_full().p_4().child(
                div()
                    .p_4()
                    .bg(cx.theme().danger.opacity(0.1))
                    .border_1()
                    .border_color(cx.theme().danger)
                    .rounded(cx.theme().radius)
                    .child(
                        Label::new(format!("Error: {}", error))
                            .text_sm()
                            .text_color(cx.theme().danger_foreground),
                    ),
            ),
        }
    }
}
