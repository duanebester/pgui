use crate::database::DatabaseManager;
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, Sizable as _, StyledExt,
    button::{Button, ButtonVariants as _},
    input::{InputState, TextInput},
    label::Label,
    v_flex,
};
use std::sync::Arc;

pub enum ConnectionEvent {
    Connected(Arc<DatabaseManager>),
    Disconnected,
    ConnectionError { field1: String },
}

impl EventEmitter<ConnectionEvent> for ConnectionsPanel {}

pub struct ConnectionsPanel {
    pub db_manager: Arc<DatabaseManager>,
    input_esc: Entity<InputState>,
    is_connected: bool,
    is_loading: bool,
}

impl ConnectionsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_esc = cx.new(|cx| {
            let mut i = InputState::new(window, cx)
                .placeholder("Enter DB URL")
                .clean_on_escape();

            i.set_value("postgres://test:test@localhost:5432/test", window, cx);
            i
        });

        Self {
            db_manager: Arc::new(DatabaseManager::new()),
            input_esc,
            is_connected: false,
            is_loading: false,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
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

    #[allow(dead_code)]
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

        v_flex()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(Label::new("Database Connection").font_bold().text_sm())
            .child(TextInput::new(&self.input_esc).cleanable())
            .child(connection_button)
    }

    fn render_connections_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .p_3()
            .flex_1()
            .child(Label::new("Saved Connections").font_bold().text_sm())
            .child(
                Label::new("Feature coming soon...")
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
    }
}

impl Render for ConnectionsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(self.render_connection_section(cx))
            .child(self.render_connections_list(cx))
    }
}
