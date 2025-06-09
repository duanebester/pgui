use std::{ops::Range, time::Duration};

use crate::database::{QueryExecutionResult, QueryResult};
use gpui::*;
use gpui_component::{
    ActiveTheme as _, h_flex, label::Label, v_flex, Size, StyleSized,
    table::{self, ColFixed, Table, TableDelegate},
};
use serde::Deserialize;

#[derive(Clone, PartialEq, Eq, Deserialize)]
struct ChangeSize(Size);

pub struct ResultsPanel {
    current_result: Option<QueryExecutionResult>,
    table: Entity<Table<ResultsTableDelegate>>,
    size: Size,
}

impl_internal_actions!(results_panel, [ChangeSize]);

struct ResultsTableDelegate {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    size: Size,
    col_resize: bool,
    col_order: bool,
    col_selection: bool,
    loading: bool,
    fixed_cols: bool,
    visible_rows: Range<usize>,
    visible_cols: Range<usize>,
}

impl ResultsTableDelegate {
    fn new() -> Self {
        Self {
            size: Size::default(),
            rows: vec![],
            columns: vec![],
            col_resize: false,
            col_order: false,
            col_selection: false,
            fixed_cols: true,
            loading: false,
            visible_cols: Range::default(),
            visible_rows: Range::default(),
        }
    }

    pub fn update(&mut self, result: QueryResult) {
        self.rows = result.rows.clone();
        self.columns = result.columns.clone();
    }
}

impl TableDelegate for ResultsTableDelegate {
    fn cols_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn col_name(&self, col_ix: usize, _: &App) -> SharedString {
        if let Some(col) = self.columns.get(col_ix) {
            col.clone().into()
        } else {
            "--".into()
        }
    }

    fn col_width(&self, col_ix: usize, _: &App) -> Pixels {
        if col_ix < 10 {
            120.0.into()
        } else if col_ix < 20 {
            80.0.into()
        } else {
            130.0.into()
        }
    }

    fn col_padding(&self, col_ix: usize, _: &App) -> Option<Edges<Pixels>> {
        if col_ix >= 3 && col_ix <= 10 {
            Some(Edges::all(px(0.)))
        } else {
            None
        }
    }

    fn col_fixed(&self, col_ix: usize, _: &App) -> Option<table::ColFixed> {
        if !self.fixed_cols {
            return None;
        }

        if col_ix < 4 {
            Some(ColFixed::Left)
        } else {
            None
        }
    }

    fn can_resize_col(&self, col_ix: usize, _: &App) -> bool {
        return self.col_resize && col_ix > 1;
    }

    fn can_select_col(&self, _: usize, _: &App) -> bool {
        return self.col_selection;
    }

    fn render_th(
        &self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<Table<Self>>,
    ) -> impl IntoElement {
        let th = div().child(self.col_name(col_ix, cx));

        if col_ix >= 3 && col_ix <= 10 {
            th.table_cell_size(self.size)
        } else {
            th
        }
    }

    fn render_tr(
        &self,
        row_ix: usize,
        _: &mut Window,
        cx: &mut Context<Table<Self>>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(row_ix)
            .on_click(cx.listener(|_, ev: &ClickEvent, _, _| {
                println!(
                    "You have clicked row with secondary: {}",
                    ev.modifiers().secondary()
                )
            }))
    }

    fn render_td(
        &self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        _cx: &mut Context<Table<Self>>,
    ) -> impl IntoElement {
        if let Some(row) = self.rows.get(row_ix) {
            if let Some(cell_value) = row.get(col_ix) {
                return cell_value.clone().into_any_element();
            }
        }

        "--".into_any_element()
    }

    fn can_loop_select(&self, _: &App) -> bool {
        false
    }

    fn can_move_col(&self, _: usize, _: &App) -> bool {
        self.col_order
    }

    fn move_col(
        &mut self,
        col_ix: usize,
        to_ix: usize,
        _: &mut Window,
        _: &mut Context<Table<Self>>,
    ) {
        let col = self.columns.remove(col_ix);
        self.columns.insert(to_ix, col);
    }

    fn loading(&self, _: &App) -> bool {
        false
    }

    fn can_load_more(&self, _: &App) -> bool {
        false
    }

    fn load_more_threshold(&self) -> usize {
        150
    }

    fn load_more(&mut self, _: &mut Window, cx: &mut Context<Table<Self>>) {
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
        _: &mut Context<Table<Self>>,
    ) {
        self.visible_rows = visible_range;
    }

    fn visible_cols_changed(
        &mut self,
        visible_range: Range<usize>,
        _: &mut Window,
        _: &mut Context<Table<Self>>,
    ) {
        self.visible_cols = visible_range;
    }
}

impl ResultsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = ResultsTableDelegate::new();
        let table = cx.new(|cx| {
            let mut t = Table::new(delegate, window, cx);
            t.set_stripe(true, cx);
            t
        });

        Self {
            current_result: None,
            table,
            size: Size::default(),
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

    fn on_change_size(&mut self, a: &ChangeSize, _: &mut Window, cx: &mut Context<Self>) {
        self.size = a.0;
        self.table.update(cx, |table, cx| {
            table.set_size(a.0, cx);
            table.delegate_mut().size = a.0;
        });
    }

    #[allow(dead_code)]
    pub fn clear_results(&mut self, cx: &mut Context<Self>) {
        self.current_result = None;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().update(
                QueryResult { columns: vec![], rows: vec![], row_count: 0, execution_time_ms: 0 }
            );
            table.refresh(cx);
        });
        cx.notify();
    }
}

impl Render for ResultsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.current_result {
            None => h_flex().size_full().items_center().justify_center().child(
                Label::new("Execute a query to see results here")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            ),
            Some(QueryExecutionResult::Select(_result)) => v_flex().on_action(cx.listener(Self::on_change_size))
                .size_full()
                .p_4()
                .child(self.table.clone()),
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
