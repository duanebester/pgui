use crate::connections_panel::ConnectionEvent;
use crate::database::{DatabaseManager, TableInfo};
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, IconName, Sizable as _, StyledExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    list::{List, ListDelegate, ListEvent, ListItem},
    v_flex,
};
use std::sync::Arc;

#[derive(IntoElement)]
struct TableListItem {
    base: ListItem,
    ix: usize,
    table: TableInfo,
    selected: bool,
}

impl TableListItem {
    pub fn new(id: impl Into<ElementId>, table: TableInfo, ix: usize, selected: bool) -> Self {
        Self {
            table,
            ix,
            base: ListItem::new(id),
            selected,
        }
    }
}

impl RenderOnce for TableListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color = if self.selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let bg_color = if self.selected {
            cx.theme().list_active
        } else if self.ix % 2 == 0 {
            cx.theme().list
        } else {
            cx.theme().list_even
        };

        let icon: Icon = match self.table.table_type.as_str() {
            "BASE TABLE" => IconName::Frame.into(),
            "VIEW" => IconName::Eye.into(),
            _ => Icon::empty().path("icons/database-zap.svg"),
        };

        self.base
            .px_3()
            .py_2()
            .overflow_x_hidden()
            .bg(bg_color)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .text_color(text_color)
                    .child(icon.size_4().text_color(text_color.opacity(0.7)))
                    .child(
                        v_flex()
                            .gap_1()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(
                                Label::new(self.table.table_name.clone())
                                    .font_medium()
                                    .whitespace_nowrap(),
                            )
                            .child(
                                Label::new(format!(
                                    "{} • {}",
                                    self.table.table_schema, self.table.table_type
                                ))
                                .text_xs()
                                .text_color(text_color.opacity(0.6))
                                .whitespace_nowrap(),
                            ),
                    ),
            )
    }
}

struct TableListDelegate {
    tables: Vec<TableInfo>,
    matched_tables: Vec<TableInfo>,
    selected_index: Option<usize>,
    query: String,
}

impl ListDelegate for TableListDelegate {
    type Item = TableListItem;

    fn items_count(&self, _: &App) -> usize {
        self.matched_tables.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) -> Task<()> {
        self.query = query.to_string();
        self.matched_tables = if query.is_empty() {
            self.tables.clone()
        } else {
            self.tables
                .iter()
                .filter(|table| {
                    table
                        .table_name
                        .to_lowercase()
                        .contains(&query.to_lowercase())
                        || table
                            .table_schema
                            .to_lowercase()
                            .contains(&query.to_lowercase())
                })
                .cloned()
                .collect()
        };
        Task::ready(())
    }

    fn confirm(&mut self, _secondary: bool, _window: &mut Window, _cx: &mut Context<List<Self>>) {
        if let Some(selected) = self.selected_index {
            if let Some(table) = self.matched_tables.get(selected) {
                println!(
                    "Selected table: {}.{}",
                    table.table_schema, table.table_name
                );
                // TODO: Emit event or callback for table selection
            }
        }
    }

    fn set_selected_index(
        &mut self,
        ix: Option<usize>,
        _: &mut Window,
        cx: &mut Context<List<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_item(
        &self,
        ix: usize,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index;
        if let Some(table) = self.matched_tables.get(ix) {
            return Some(TableListItem::new(ix, table.clone(), ix, selected));
        }
        None
    }

    fn loading(&self, _: &App) -> bool {
        false // We don't have pagination for tables
    }

    fn can_load_more(&self, _: &App) -> bool {
        false // No pagination needed for tables
    }

    fn load_more_threshold(&self) -> usize {
        0
    }

    fn load_more(&mut self, _window: &mut Window, _cx: &mut Context<List<Self>>) {
        // No-op for tables
    }
}

impl TableListDelegate {
    fn new() -> Self {
        Self {
            tables: Vec::new(),
            matched_tables: Vec::new(),
            selected_index: None,
            query: String::new(),
        }
    }

    fn update_tables(&mut self, tables: Vec<TableInfo>) {
        self.tables = tables;
        self.matched_tables = self.tables.clone();
        if !self.matched_tables.is_empty() && self.selected_index.is_none() {
            self.selected_index = Some(0);
        }
    }

    #[allow(dead_code)]
    fn selected_table(&self) -> Option<&TableInfo> {
        self.selected_index
            .and_then(|ix| self.matched_tables.get(ix))
    }
}

pub struct TablesPanel {
    table_list: Entity<List<TableListDelegate>>,
    db_manager: Option<Arc<DatabaseManager>>,
    is_connected: bool,
    _subscriptions: Vec<Subscription>,
}

impl TablesPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let table_list = cx.new(|cx| List::new(TableListDelegate::new(), window, cx));

        let _subscriptions = vec![
            cx.subscribe(&table_list, |_, _, ev: &ListEvent, _| match ev {
                ListEvent::Select(ix) => {
                    println!("Table selected: {:?}", ix);
                }
                ListEvent::Confirm(ix) => {
                    println!("Table confirmed: {:?}", ix);
                }
                ListEvent::Cancel => {
                    println!("Table selection cancelled");
                }
            }),
        ];

        Self {
            table_list,
            db_manager: None,
            is_connected: false,
            _subscriptions,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn handle_connection_event(&mut self, event: &ConnectionEvent, cx: &mut Context<Self>) {
        match event {
            ConnectionEvent::Connected(db_manager) => {
                self.db_manager = Some(db_manager.clone());
                self.is_connected = true;
                self.load_tables(cx);
            }
            ConnectionEvent::Disconnected => {
                self.db_manager = None;
                self.is_connected = false;
                self.clear_tables(cx);
            }
            ConnectionEvent::ConnectionError { field1: _str } => {
                self.db_manager = None;
                self.is_connected = false;
                self.clear_tables(cx);
            }
        }
    }

    fn load_tables(&mut self, cx: &mut Context<Self>) {
        if !self.is_connected {
            return;
        }

        let Some(db_manager) = self.db_manager.clone() else {
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = db_manager.get_tables().await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(tables) => {
                        this.table_list.update(cx, |list, cx| {
                            list.delegate_mut().update_tables(tables);
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to load tables: {}", e);
                        this.table_list.update(cx, |list, cx| {
                            list.delegate_mut().update_tables(Vec::new());
                            cx.notify();
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn clear_tables(&mut self, cx: &mut Context<Self>) {
        self.table_list.update(cx, |list, cx| {
            list.delegate_mut().update_tables(Vec::new());
            cx.notify();
        });
    }

    pub fn refresh_tables(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.load_tables(cx);
    }
}

impl Render for TablesPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let refresh_button = Button::new("refresh")
            .icon(Icon::empty().path("icons/rotate-ccw.svg"))
            .small()
            .ghost()
            .disabled(!self.is_connected)
            .on_click(cx.listener(Self::refresh_tables));

        let header = h_flex()
            .justify_between()
            .items_center()
            .child(Label::new("Tables").font_bold().text_sm())
            .child(refresh_button);

        let table_count = self.table_list.read(cx).delegate().matched_tables.len();
        let status_text = if !self.is_connected {
            "Connect to database to view tables".to_string()
        } else if table_count == 0 {
            "No tables found".to_string()
        } else {
            format!(
                "{} table{}",
                table_count,
                if table_count == 1 { "" } else { "s" }
            )
        };

        v_flex()
            .size_full()
            .gap_2()
            .p_3()
            .bg(cx.theme().background)
            .child(header)
            .child(
                Label::new(status_text)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .overflow_hidden()
                    .child(self.table_list.clone()),
            )
    }
}
