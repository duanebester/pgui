use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, IndexPath, Selectable, StyledExt, h_flex,
    label::Label,
    list::{List, ListDelegate, ListItem},
    v_flex,
};

use crate::{services::*, state::ConnectionState};

#[derive(IntoElement)]
struct ConnectionListItem {
    base: ListItem,
    ix: IndexPath,
    connection: ConnectionInfo,
    selected: bool,
}

impl ConnectionListItem {
    pub fn new(
        id: impl Into<ElementId>,
        connection: ConnectionInfo,
        ix: IndexPath,
        selected: bool,
    ) -> Self {
        Self {
            connection,
            ix,
            base: ListItem::new(id),
            selected,
        }
    }
}

impl Selectable for ConnectionListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for ConnectionListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color = if self.selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let bg_color = if self.selected {
            cx.theme().list_active
        } else if self.ix.row % 2 == 0 {
            cx.theme().list
        } else {
            cx.theme().list_even
        };

        self.base
            .px_3()
            .py_2()
            .overflow_x_hidden()
            .bg(bg_color)
            .when(self.selected, |this| {
                this.border_color(cx.theme().list_active_border)
            })
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .text_color(text_color)
                    .child(
                        v_flex()
                            .gap_1()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(
                                Label::new(self.connection.name.clone())
                                    .font_semibold()
                                    .whitespace_nowrap(),
                            )
                            .child(
                                Label::new(format!(
                                    "{}@{}:{}/{}",
                                    self.connection.username,
                                    self.connection.hostname,
                                    self.connection.port,
                                    self.connection.database
                                ))
                                .text_xs()
                                .text_color(text_color.opacity(0.6))
                                .whitespace_nowrap(),
                            ),
                    ),
            )
    }
}

struct ConnectionListDelegate {
    connections: Vec<ConnectionInfo>,
    matched_connections: Vec<ConnectionInfo>,
    selected_index: Option<IndexPath>,
    query: String,
}

impl ListDelegate for ConnectionListDelegate {
    type Item = ConnectionListItem;

    fn items_count(&self, _section: usize, _app: &App) -> usize {
        self.matched_connections.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) -> Task<()> {
        self.query = query.to_string();
        self.matched_connections = if query.is_empty() {
            self.connections.clone()
        } else {
            self.connections
                .iter()
                .filter(|conn| {
                    conn.database.to_lowercase().contains(&query.to_lowercase())
                        || conn.username.to_lowercase().contains(&query.to_lowercase())
                })
                .cloned()
                .collect()
        };
        Task::ready(())
    }

    fn confirm(&mut self, _secondary: bool, _window: &mut Window, cx: &mut Context<List<Self>>) {
        if let Some(selected) = self.selected_index {
            if let Some(conn) = self.matched_connections.get(selected.row) {
                println!("Selected connection: {}@{}", conn.username, conn.hostname);
                ConnectionState::set_active(&conn, cx);
            }
        }
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<List<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_item(
        &self,
        ix: IndexPath,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index;
        if let Some(conn) = self.matched_connections.get(ix.row) {
            return Some(ConnectionListItem::new(ix, conn.clone(), ix, selected));
        }
        None
    }
}

impl ConnectionListDelegate {
    fn new() -> Self {
        Self {
            connections: vec![],
            matched_connections: vec![],
            selected_index: None,
            query: String::new(),
        }
    }

    fn update_connections(&mut self, connections: Vec<ConnectionInfo>) {
        self.connections = connections;
        self.matched_connections = self.connections.clone();
        if !self.matched_connections.is_empty() && self.selected_index.is_none() {
            self.selected_index = Some(IndexPath::default());
        }
    }
}

pub struct ConnectionsPanel {
    connection_list: Entity<List<ConnectionListDelegate>>,
    _subscriptions: Vec<Subscription>,
}

impl ConnectionsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let connection_list = cx.new(|cx| List::new(ConnectionListDelegate::new(), window, cx));

        let conn_list_clone = connection_list.clone();
        let _subscriptions = vec![cx.observe_global::<ConnectionState>(move |_this, cx| {
            let conns = cx.global::<ConnectionState>().saved_connections.clone();
            let _ = cx.update_entity(&conn_list_clone, |list, cx| {
                list.delegate_mut().update_connections(conns);
                cx.notify();
            });

            cx.notify();
        })];

        Self {
            connection_list,
            _subscriptions,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn render_connections_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .p_2()
            .flex_1()
            .items_start()
            .child(Label::new("Connections").font_bold().text_base().pl_1())
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .overflow_hidden()
                    .child(self.connection_list.clone()),
            )
    }
}

impl Render for ConnectionsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().sidebar_primary_foreground)
            .child(self.render_connections_list(cx))
    }
}
