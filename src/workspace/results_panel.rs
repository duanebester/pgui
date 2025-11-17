use std::ops::Range;

use crate::services::{EnhancedQueryExecutionResult, EnhancedQueryResult, ResultCell};
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Size, h_flex,
    label::Label,
    table::{Column, Table, TableDelegate, TableState},
    v_flex,
};
use serde::Deserialize;

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = enhanced_results_panel, no_json)]
struct ChangeSize(Size);

pub struct EnhancedResultsPanel {
    current_result: Option<EnhancedQueryExecutionResult>,
    table: Entity<TableState<EnhancedResultsTableDelegate>>,
}

struct EnhancedResultsTableDelegate {
    columns: Vec<Column>,
    // Store the full ResultCell data with metadata
    rows: Vec<Vec<ResultCell>>,
    loading: bool,
    visible_rows: Range<usize>,
}

impl EnhancedResultsTableDelegate {
    fn new() -> Self {
        Self {
            rows: vec![],
            columns: vec![],
            loading: false,
            visible_rows: Range::default(),
        }
    }

    pub fn update(&mut self, result: EnhancedQueryResult) {
        // Convert ResultRows to Vec<Vec<ResultCell>>
        let rows: Vec<Vec<ResultCell>> = result
            .rows
            .clone()
            .iter()
            .map(|row| row.cells.clone())
            .collect();

        // Create columns from metadata
        let columns: Vec<Column> = result
            .columns
            .clone()
            .iter()
            .map(|col_meta| {
                Column::new(&col_meta.name, &col_meta.name).sortable() // Enable sorting for all columns
            })
            .collect();

        self.rows = rows;
        self.columns = columns;
    }
}

impl TableDelegate for EnhancedResultsTableDelegate {
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
        let col = self.column(col_ix, cx);
        div().child(format!("{}", col.clone().name))
        // let col_meta = if !self.rows.is_empty() && col_ix < self.rows[0].len() {
        //     Some(&self.rows[0][col_ix].column_metadata)
        // } else {
        //     None
        // };

        // let mut th = div();

        // if let Some(meta) = col_meta {
        //     // Show column name and type
        //     th = th.child(
        //         v_flex()
        //             .gap_1()
        //             .child(
        //                 Label::new(&meta.name)
        //                     .text_sm()
        //                     .text_color(cx.theme().foreground),
        //             )
        //             .child(
        //                 Label::new(&meta.type_name)
        //                     .text_xs()
        //                     .text_color(cx.theme().muted_foreground),
        //             ),
        //     );
        // } else {
        //     th = th.child(format!("{}", col.name));
        // }

        // th
    }

    fn render_tr(&self, row_ix: usize, _: &mut Window, _cx: &mut App) -> gpui::Stateful<gpui::Div> {
        div().id(row_ix).on_click(move |ev: &ClickEvent, _, _| {
            println!(
                "You have clicked row {} with secondary: {}",
                row_ix,
                ev.modifiers().secondary()
            );
        })
    }

    fn render_td(
        &self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        // println!("render_td called: row={}, col={}", row_ix, col_ix);
        // Don't clone all rows - access directly instead
        if let Some(row) = self.rows.get(row_ix) {
            if let Some(cell) = row.get(col_ix) {
                // Only clone the specific cell we need for the closure
                let cell_clone = cell.clone();
                // Create a clickable cell that logs metadata on click
                return div()
                    .cursor_pointer()
                    .on_mouse_up(MouseButton::Left, move |_ev, _, _| {
                        // Log all the metadata for this cell
                        println!("\n=== CELL METADATA ===");
                        println!("Column Name: {}", cell_clone.column_metadata.name);
                        println!("Column Type: {}", cell_clone.column_metadata.type_name);
                        println!("Column Ordinal: {}", cell_clone.column_metadata.ordinal);
                        println!("Table Name: {:?}", cell_clone.column_metadata.table_name);
                        println!("Is Nullable: {:?}", cell_clone.column_metadata.is_nullable);
                        println!("Value: {}", cell_clone.value);
                        println!("Is NULL: {}", cell_clone.is_null);
                        println!("====================\n");
                    })
                    .child(if cell.is_null {
                        // Style NULL values differently
                        Label::new(&cell.value)
                            .text_color(cx.theme().muted_foreground)
                            .italic()
                    } else {
                        Label::new(&cell.value)
                    })
                    .into_any_element();
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

        // Also move the cells in each row
        for row in &mut self.rows {
            if col_ix < row.len() && to_ix < row.len() {
                let cell = row.remove(col_ix);
                row.insert(to_ix, cell);
            }
        }
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn load_more_threshold(&self) -> usize {
        150
    }

    // fn load_more(&mut self, _: &mut Window, cx: &mut Context<TableState<Self>>) {
    //     self.loading = true;
    //     cx.spawn(async move |view, cx| {
    //         // Simulate network request
    //         Timer::after(Duration::from_secs(1)).await;
    //         cx.update(|cx| {
    //             let _ = view.update(cx, |view, _| {
    //                 view.delegate_mut().loading = false;
    //             });
    //         })
    //     })
    //     .detach();
    // }

    fn visible_rows_changed(
        &mut self,
        visible_range: Range<usize>,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        self.visible_rows = visible_range;
    }
}

impl EnhancedResultsPanel {
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

    pub fn update_result(&mut self, result: EnhancedQueryExecutionResult, cx: &mut Context<Self>) {
        self.current_result = Some(result.clone());
        if let EnhancedQueryExecutionResult::Select(x) = result {
            self.table.update(cx, |table, cx| {
                table.delegate_mut().update(x.clone());
                table.refresh(cx);
            });
        }
        cx.notify();
    }
}

impl Render for EnhancedResultsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.current_result {
            Some(EnhancedQueryExecutionResult::Select(_result)) => v_flex()
                .size_full()
                .p_4()
                .child(Table::new(&self.table.clone()).stripe(true)),
            Some(EnhancedQueryExecutionResult::Modified {
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
            Some(EnhancedQueryExecutionResult::Error(error)) => v_flex().size_full().p_4().child(
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
