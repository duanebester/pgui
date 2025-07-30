use std::sync::Arc;

use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, Selectable, Sizable as _, StyledExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputState, TextInput},
    label::Label,
    list::{List, ListDelegate, ListEvent, ListItem},
    v_flex,
};
use serde::{Deserialize, Serialize};

use crate::services::{DatabaseManager, load_connections, save_connections, get_connections_file_path, parse_connection_url, connection_to_url};

pub enum ConnectionEvent {
    Connected(Arc<DatabaseManager>),
    Disconnected,
    ConnectionError { field1: String },
}

impl EventEmitter<ConnectionEvent> for ConnectionsPanel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub hostname: String,
    pub username: String,
    pub password: String,
    pub database: String,
    pub port: String,
}

#[derive(IntoElement)]
struct ConnectionListItem {
    base: ListItem,
    ix: usize,
    connection: ConnectionInfo,
    selected: bool,
}

impl ConnectionListItem {
    pub fn new(
        id: impl Into<ElementId>,
        connection: ConnectionInfo,
        ix: usize,
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
            cx.theme().list_active.opacity(0.2)
        } else if self.ix % 2 == 0 {
            cx.theme().list
        } else {
            cx.theme().list_even
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
                    .child(
                        v_flex()
                            .gap_1()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(
                                Label::new(self.connection.database.clone())
                                    .font_medium()
                                    .whitespace_nowrap(),
                            )
                            .child(
                                Label::new(format!(
                                    "{}@{}:{}",
                                    self.connection.username,
                                    self.connection.hostname,
                                    self.connection.port
                                ))
                                .text_xs()
                                .text_color(text_color.opacity(0.6))
                                .whitespace_nowrap(),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new(format!("save_{}", self.ix))
                                    .icon(Icon::empty().path("icons/save.svg"))
                                    .ghost()
                                    .small()
                                    .tooltip("Save Connection")
                            )
                            .child(
                                Button::new(format!("delete_{}", self.ix))
                                    .icon(Icon::empty().path("icons/trash.svg"))
                                    .ghost()
                                    .small()
                                    .danger()
                                    .tooltip("Delete Connection")
                            )
                    ),
            )
    }
}

struct ConnectionListDelegate {
    connections: Vec<ConnectionInfo>,
    matched_connections: Vec<ConnectionInfo>,
    selected_index: Option<usize>,
    query: String,
}

impl ListDelegate for ConnectionListDelegate {
    type Item = ConnectionListItem;

    fn items_count(&self, _: &App) -> usize {
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
                        || conn.hostname.to_lowercase().contains(&query.to_lowercase())
                })
                .cloned()
                .collect()
        };
        Task::ready(())
    }

    fn confirm(&mut self, _secondary: bool, _window: &mut Window, _cx: &mut Context<List<Self>>) {
        if let Some(selected) = self.selected_index {
            if let Some(conn) = self.matched_connections.get(selected) {
                println!("Selected conn: {}@{}", conn.username, conn.hostname);
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
        if let Some(conn) = self.matched_connections.get(ix) {
            return Some(ConnectionListItem::new(ix, conn.clone(), ix, selected));
        }
        None
    }

    fn loading(&self, _: &App) -> bool {
        false
    }

    fn can_load_more(&self, _: &App) -> bool {
        false
    }

    fn load_more_threshold(&self) -> usize {
        0
    }

    fn load_more(&mut self, _window: &mut Window, _cx: &mut Context<List<Self>>) {
        // No-op
    }
}

impl ConnectionListDelegate {
    fn new() -> Self {
        Self {
            connections: Vec::new(),
            matched_connections: Vec::new(),
            selected_index: None,
            query: String::new(),
        }
    }

    fn update_connections(&mut self, connections: Vec<ConnectionInfo>) {
        self.connections = connections;
        self.matched_connections = self.connections.clone();
        if !self.matched_connections.is_empty() && self.selected_index.is_none() {
            self.selected_index = Some(0);
        }
    }

    fn selected_connection(&self) -> Option<&ConnectionInfo> {
        self.selected_index
            .and_then(|ix| self.matched_connections.get(ix))
    }
}

pub struct ConnectionsPanel {
    pub db_manager: Arc<DatabaseManager>,
    input_esc: Entity<InputState>,
    connection_list: Entity<List<ConnectionListDelegate>>,
    is_connected: bool,
    is_loading: bool,
    _subscriptions: Vec<Subscription>,
}

impl ConnectionsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_esc = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter DB URL (postgresql://user:pass@host:port/db)")
                .clean_on_escape()
        });

        let connection_list = cx.new(|cx| List::new(ConnectionListDelegate::new(), window, cx));

        let _subscriptions = vec![cx.subscribe_in(
            &connection_list,
            window,
            |this, _, ev: &ListEvent, window, cx| match ev {
                ListEvent::Confirm(ix) => {
                    if let Some(conn) = this.get_selected_connection(*ix, cx) {
                        let con_str = connection_to_url(&conn);
                        this.input_esc.update(cx, |is, cx| {
                            is.set_value(con_str, window, cx);
                            cx.notify();
                        })
                    }
                }
                _ => {}
            },
        )];

        // Load saved connections
        let connection_list_entity = connection_list.clone();
        cx.spawn(async move |_view, cx| {
            if let Ok(file_path) = get_connections_file_path() {
                if let Ok(connections) = load_connections(&file_path).await {
                    connection_list_entity.update(cx, |list, cx| {
                        list.delegate_mut().update_connections(connections);
                        cx.notify();
                    }).ok();
                }
            }
        })
        .detach();

        Self {
            db_manager: Arc::new(DatabaseManager::new()),
            input_esc,
            is_connected: false,
            is_loading: false,
            connection_list,
            _subscriptions,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn get_selected_connection(&self, ix: usize, cx: &App) -> Option<ConnectionInfo> {
        self.connection_list
            .read(cx)
            .delegate()
            .matched_connections
            .get(ix)
            .cloned()
    }

    pub fn connect_to_database(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_loading {
            return;
        }

        self.is_loading = true;
        cx.notify();

        let db_manager = self.db_manager.clone();
        let connection_url = self.input_esc.read(cx).value().clone();

        cx.spawn(async move |this: WeakEntity<ConnectionsPanel>, cx| {
            let result = db_manager.connect(&connection_url).await;

            this.update(cx, |this, cx| {
                this.is_loading = false;
                match result {
                    Ok(_) => {
                        this.is_connected = true;
                        cx.emit(ConnectionEvent::Connected(this.db_manager.clone()));
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to connect to database: {}", e);
                        eprintln!("{}", error_msg);
                        this.is_connected = false;
                        cx.emit(ConnectionEvent::ConnectionError { field1: error_msg });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn disconnect_from_database(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let db_manager = self.db_manager.clone();

        cx.spawn(async move |this, cx| {
            db_manager.disconnect().await;

            this.update(cx, |this, cx| {
                this.is_connected = false;
                cx.emit(ConnectionEvent::Disconnected);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn save_current_connection(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let connection_url = self.input_esc.read(cx).value().clone();
        
        if connection_url.trim().is_empty() {
            return;
        }

        let connection_list_entity = self.connection_list.clone();
        cx.spawn(async move |_this, cx| {
            // Parse the connection URL
            if let Ok(connection_info) = parse_connection_url(&connection_url) {
                // Save to file
                if let Ok(file_path) = get_connections_file_path() {
                    if let Ok(mut connections) = load_connections(&file_path).await {
                        // Check if connection already exists
                        let exists = connections.iter().any(|conn| {
                            conn.hostname == connection_info.hostname 
                                && conn.username == connection_info.username 
                                && conn.database == connection_info.database
                                && conn.port == connection_info.port
                        });

                        if !exists {
                            connections.push(connection_info);
                            if let Ok(()) = save_connections(&file_path, &connections).await {
                                // Update the UI list
                                connection_list_entity.update(cx, |list, cx| {
                                    list.delegate_mut().update_connections(connections);
                                    cx.notify();
                                }).ok();
                            }
                        }
                    }
                }
            }
        })
        .detach();
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    fn render_connection_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let connection_button = if self.is_connected {
            Button::new("disconnect")
                .label("Disconnect")
                .icon(Icon::empty().path("icons/unplug.svg"))
                .small()
                .danger()
                .on_click(cx.listener(Self::disconnect_from_database))
        } else {
            Button::new("connect")
                .label(if self.is_loading {
                    "Connecting..."
                } else {
                    "Connect"
                })
                .icon(Icon::empty().path("icons/plug-zap.svg"))
                .small()
                .outline()
                .disabled(self.is_loading)
                .on_click(cx.listener(Self::connect_to_database))
        };

        let save_button = Button::new("save_connection")
            .label("Save")
            .icon(Icon::empty().path("icons/save.svg"))
            .small()
            .ghost()
            .on_click(cx.listener(Self::save_current_connection));

        v_flex()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(Label::new("Database Connection").font_bold().text_sm())
            .child(TextInput::new(&self.input_esc).cleanable())
            .child(
                h_flex()
                    .gap_2()
                    .child(connection_button)
                    .child(save_button)
            )
    }

    fn render_connections_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .p_3()
            .flex_1()
            .child(Label::new("Saved Connections").font_bold().text_sm())
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
            .child(self.render_connection_section(cx))
            .child(self.render_connections_list(cx))
    }
}