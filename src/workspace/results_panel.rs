use std::{ops::Range, time::Duration};

use crate::services::{QueryExecutionResult, QueryResult};
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Size, StyleSized, h_flex,
    label::Label,
    table::{Column, Table, TableDelegate, TableState},
    v_flex,
};
use serde::Deserialize;

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = results_panel, no_json)]
struct ChangeSize(Size);

pub struct ResultsPanel {
    current_result: Option<QueryExecutionResult>,
    table: Entity<TableState<ResultsTableDelegate>>,
}

struct ResultsTableDelegate {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
    size: Size,
    loading: bool,
    visible_rows: Range<usize>,
}

impl ResultsTableDelegate {
    fn new() -> Self {
        Self {
            size: Size::default(),
            rows: vec![],
            columns: vec![],
            loading: false,
            visible_rows: Range::default(),
        }
    }

    pub fn update(&mut self, result: QueryResult) {
        self.rows = result.rows.clone();
        let columns: Vec<Column> = result
            .columns
            .clone()
            .iter()
            .map(|c| Column::new(c, c)) // TODO: Create pretty column name
            .collect();
        self.columns = columns;
    }
}

impl TableDelegate for ResultsTableDelegate {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> &Column {
        self.columns.get(col_ix).unwrap()
    }

    fn render_th(&self, col_ix: usize, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let th = div().child(format!("{}", self.column(col_ix, cx).name));

        if col_ix >= 3 && col_ix <= 10 {
            th.table_cell_size(self.size)
        } else {
            th
        }
    }

    fn render_tr(&self, row_ix: usize, _: &mut Window, _cx: &mut App) -> gpui::Stateful<gpui::Div> {
        div().id(row_ix).on_click(|ev: &ClickEvent, _, _| {
            println!(
                "You have clicked row with secondary: {}",
                ev.modifiers().secondary()
            )
        })
    }

    fn render_td(
        &self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        _: &mut App,
    ) -> impl IntoElement {
        if let Some(row) = self.rows.get(row_ix) {
            if let Some(cell_value) = row.get(col_ix) {
                return cell_value.clone().into_any_element();
            }
        }

        "--".into_any_element()
    }

    fn move_column(
        &mut self,
        col_ix: usize,
        to_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        let col = self.columns.remove(col_ix);
        self.columns.insert(to_ix, col);
    }

    fn loading(&self, _: &App) -> bool {
        false
    }

    fn load_more_threshold(&self) -> usize {
        150
    }

    fn load_more(&mut self, _: &mut Window, cx: &mut Context<TableState<Self>>) {
        self.loading = true;
        cx.spawn(async move |view, cx| {
            // Simulate network request, delay 1s to load data.
            Timer::after(Duration::from_secs(1)).await;
            cx.update(|cx| {
                let _ = view.update(cx, |view, _| {
                    view.delegate_mut().loading = false;
                });
            })
        })
        .detach();
    }

    fn visible_rows_changed(
        &mut self,
        visible_range: Range<usize>,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        self.visible_rows = visible_range;
    }
}

impl ResultsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = ResultsTableDelegate::new();
        let table = cx.new(|cx| TableState::new(delegate, window, cx));

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
            Some(QueryExecutionResult::Modified {
                rows_affected,
                execution_time_ms,
            }) => h_flex().size_full().items_center().justify_center().child(
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
                    .bg(cx.theme().danger)
                    .border_1()
                    .border_color(cx.theme().danger)
                    .rounded(cx.theme().radius)
                    .child(
                        Label::new(format!("Error: {}", error))
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
