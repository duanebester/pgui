use gpui::{
    App, AppContext, ClickEvent, Context, Entity, EventEmitter, InteractiveElement, ParentElement,
    Render, Styled, Subscription, Window, actions, div, px,
};

use gpui_component::{
    ActiveTheme as _, Disableable, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    list::ListItem,
    tree::{TreeEntry, TreeItem, TreeState, tree},
    v_flex,
};

use crate::{
    services::{ConnectionInfo, DatabaseManager, TableInfo},
    state::ConnectionState,
};

pub enum TableEvent {
    TableSelected(TableInfo),
}

impl EventEmitter<TableEvent> for TablesTree {}

actions!(tables_tree, [SelectItem]);

pub struct TablesTree {
    tree_state: Entity<TreeState>,
    selected_item: Option<TreeItem>,
    db_manager: Option<DatabaseManager>,
    active_connection: Option<ConnectionInfo>,
    _subscriptions: Vec<Subscription>,
}

fn build_tree_items(tables: Vec<TableInfo>) -> Vec<TreeItem> {
    use std::collections::HashMap;

    // Group tables by schema
    let mut schema_map: HashMap<String, Vec<TableInfo>> = HashMap::new();
    for table in tables {
        schema_map
            .entry(table.table_schema.clone())
            .or_insert_with(Vec::new)
            .push(table);
    }

    // Convert to sorted vec of (schema, tables)
    let mut schemas: Vec<(String, Vec<TableInfo>)> = schema_map.into_iter().collect();
    schemas.sort_by(|a, b| a.0.cmp(&b.0));

    // Build tree items with schema -> tables hierarchy
    schemas
        .into_iter()
        .map(|(schema, mut tables)| {
            // Sort tables within each schema
            tables.sort_by(|a, b| a.table_name.cmp(&b.table_name));

            // Create table items
            let table_items: Vec<TreeItem> = tables
                .into_iter()
                .map(|t| {
                    TreeItem::new(
                        format!("{}.{}-{}", schema, t.table_name, t.table_type), // id
                        t.table_name,                                            // label
                    )
                })
                .collect();

            // Create schema item with tables as children
            TreeItem::new(format!("{}-schema", schema.clone()), schema)
                .expanded(true)
                .children(table_items)
        })
        .collect()
}

impl TablesTree {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn load_tables(&mut self, cx: &mut Context<Self>) {
        if self.active_connection.is_none() {
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
                        let items = build_tree_items(tables);
                        this.tree_state.update(cx, |state, cx| {
                            state.set_items(items, cx);
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to load tables: {}", e);
                        this.tree_state.update(cx, |state, cx| {
                            state.set_items(vec![], cx);
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
        self.tree_state.update(cx, |state, cx| {
            state.set_items(vec![], cx);
            cx.notify();
        });
    }

    pub fn refresh_tables(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.load_tables(cx);
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tree_state = cx.new(|cx| TreeState::new(cx));

        let _subscriptions =
            vec![
                cx.observe_global_in::<ConnectionState>(window, move |this, _win, cx| {
                    let state = cx.global::<ConnectionState>();
                    let active_connection = state.active_connection.clone();

                    this.db_manager = Some(state.db_manager.clone());
                    this.active_connection = active_connection.clone();
                    if active_connection.is_some() {
                        this.load_tables(cx);
                    } else {
                        this.clear_tables(cx);
                    }

                    cx.notify();
                }),
            ];

        Self {
            tree_state,
            selected_item: None,
            db_manager: None,
            active_connection: None,
            _subscriptions,
        }
    }

    fn on_select_table_item(
        &mut self,
        _: &SelectItem,
        _: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Some(entry) = self.tree_state.read(cx).selected_entry() {
            self.selected_item = Some(entry.item().clone());
            let item = entry.item();
            // Parse the id format: "{schema}.{table_name}-{table_type}"
            let parts: Vec<&str> = item.id.rsplitn(2, '-').collect();
            if parts.len() == 2 {
                let table_type = parts[0].to_string();
                let schema_and_table: Vec<&str> = parts[1].splitn(2, '.').collect();

                if schema_and_table.len() == 2 {
                    let table_schema = schema_and_table[0].to_string();
                    let table_name = schema_and_table[1].to_string();

                    let table_info = TableInfo {
                        table_schema,
                        table_name,
                        table_type,
                    };
                    cx.emit(TableEvent::TableSelected(table_info));
                }
            }
            cx.notify();
        }
    }

    fn render_tree_item(
        &self,
        ix: usize,
        entry: &TreeEntry,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> ListItem {
        let item = entry.item();
        let is_selected = selected;

        let name = truncate(item.label.clone().as_str(), 23);

        let table_type = if item.id.clone().ends_with("-VIEW") {
            "VIEW"
        } else if item.id.clone().ends_with("-BASE TABLE") {
            "BASE"
        } else {
            "SCHEMA"
        };

        // Determine colors based on selection state
        let text_color = if is_selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let bg_color = if is_selected {
            cx.theme().list_active
        } else if ix % 2 == 0 {
            cx.theme().list
        } else {
            cx.theme().list_even
        };

        // Icon based on item type
        let icon = if !entry.is_folder() {
            // check if id ends with -view
            if item.id.clone().ends_with("-VIEW") {
                IconName::Eye
            } else {
                IconName::Frame
            }
        } else if entry.is_expanded() {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };

        let icon: Icon = icon.into();

        ListItem::new(ix)
            .w_full()
            .py_3()
            .px_4()
            .pl(px(16.) * entry.depth() + px(12.))
            .bg(bg_color)
            .border_1()
            .border_color(if is_selected {
                cx.theme().list_active_border
            } else {
                bg_color
            })
            .rounded(cx.theme().radius)
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .text_color(text_color)
                            .child(icon.size_4().text_color(text_color.opacity(0.7)))
                            .child(Label::new(name).font_medium().text_sm().whitespace_nowrap()),
                    )
                    .child(
                        Label::new(table_type)
                            .text_xs()
                            .text_color(text_color.opacity(0.6)),
                    ),
            )
            .on_click(cx.listener({
                let item = item.clone();
                move |this, _, window, cx| {
                    this.selected_item = Some(item.clone());
                    this.on_select_table_item(&SelectItem, window, cx);
                    cx.notify();
                }
            }))
    }
}

impl Render for TablesTree {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let view = cx.entity();

        let refresh_button = Button::new("refresh")
            .icon(Icon::empty().path("icons/rotate-ccw.svg"))
            .small()
            .ghost()
            .tooltip("Refresh Tables")
            .disabled(self.active_connection.clone().is_none())
            .on_click(cx.listener(Self::refresh_tables));

        let header = div().child(
            div()
                .h_flex()
                .justify_between()
                .items_center()
                .child(Label::new("Tables").font_bold().text_base())
                .child(refresh_button),
        );

        v_flex()
            .flex_1()
            .gap_2()
            .p_2()
            .on_action(cx.listener(Self::on_select_table_item))
            .child(header)
            .child(
                tree(&self.tree_state, move |ix, entry, selected, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.render_tree_item(ix, entry, selected, cx)
                    })
                })
                .p(px(8.))
                .flex_1()
                .w_full()
                .border_1()
                .border_color(cx.theme().border)
                .rounded(cx.theme().radius),
            )
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
