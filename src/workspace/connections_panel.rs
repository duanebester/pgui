use crate::{
    services::{DatabaseManager, SavedConnection},
    state::{AppState},
};
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants as _}, form::{form_field, v_form}, h_flex, input::{InputState, TextInput}, label::Label, v_flex, ActiveTheme as _, ContextModal as _, Disableable, Icon, IconName, Placement, Sizable as _, StyledExt
};
use std::sync::Arc;

pub enum ConnectionEvent {
    Connected(Arc<DatabaseManager>),
    Disconnected,
    ConnectionError { field1: String },
}

impl EventEmitter<ConnectionEvent> for ConnectionsPanel {}

pub struct ConnectionDrawer {
    focus_handle: FocusHandle,
    placement: Placement,
    name_input: Entity<InputState>,
    username_input: Entity<InputState>,
    password_input: Entity<InputState>,
    hostname_input: Entity<InputState>,
    database_input: Entity<InputState>,
    port_input: Entity<InputState>,
    modal_overlay: bool,
    model_show_close: bool,
    model_padding: bool,
    model_keyboard: bool,
    overlay_closable: bool,
}

impl ConnectionDrawer {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx).default_value("Local Dev"));
        let username_input = cx.new(|cx| InputState::new(window, cx).default_value("test"));
        let password_input = cx.new(|cx| InputState::new(window, cx).default_value("test"));
        let hostname_input = cx.new(|cx| InputState::new(window, cx).default_value("localhost"));
        let database_input = cx.new(|cx| InputState::new(window, cx).default_value("test"));
        let port_input = cx.new(|cx| InputState::new(window, cx).default_value("5432"));

        Self {
            focus_handle: cx.focus_handle(),
            placement: Placement::Right,
            name_input,
            username_input,
            password_input,
            hostname_input,
            database_input,
            port_input,
            modal_overlay: true,
            model_show_close: true,
            model_padding: true,
            model_keyboard: true,
            overlay_closable: true,
        }
    }

    fn open_drawer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let overlay = self.modal_overlay;
        let name_input = self.name_input.clone();
        let username_input = self.username_input.clone();
        let password_input = self.password_input.clone();
        let hostname_input = self.hostname_input.clone();
        let database_input = self.database_input.clone();
        let port_input = self.port_input.clone();

        window.open_drawer_at(self.placement, cx, move |this, _, cx| {
            this.overlay(overlay)
                .size(px(400.))
                .title("Add Connection")
                .gap_4()
                .child(TextInput::new(&name_input))
                .child(TextInput::new(&username_input))
                .child(TextInput::new(&password_input))
                .child(TextInput::new(&hostname_input))
                .child(TextInput::new(&database_input))
                .child(TextInput::new(&port_input))
                .footer(
                    h_flex()
                        .gap_6()
                        .items_center()
                        .child(Button::new("confirm").primary().label("Confirm").on_click(
                            |_, window, cx| {
                                window.close_drawer(cx);
                            },
                        ))
                        .child(
                            Button::new("cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| {
                                    window.close_drawer(cx);
                                }),
                        ),
                )
        });
    }
}

impl Focusable for ConnectionDrawer {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}


pub struct ConnectionsPanel {
    pub db_manager: Arc<DatabaseManager>,
    connection_drawer: Entity<ConnectionDrawer>,
    is_connected: bool,
    is_loading: bool,
    saved_connections: Vec<SavedConnection>,
    modal_overlay: bool,
    model_show_close: bool,
    model_padding: bool,
    model_keyboard: bool,
    overlay_closable: bool,
}

impl ConnectionsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let connections = cx.global::<AppState>().saved_connections.clone();
        cx.observe(&connections, |this, saved_connections, cx| {
            let connections = saved_connections.read(cx).connections.clone();
            this.saved_connections = connections;
            cx.notify();
        })
        .detach();

        let connection_drawer = ConnectionDrawer::view(window, cx);

        Self {
            db_manager: Arc::new(DatabaseManager::new()),
            is_connected: false,
            is_loading: false,
            saved_connections: vec![],
            connection_drawer,
            modal_overlay: true,
            model_show_close: true,
            model_padding: true,
            model_keyboard: true,
            overlay_closable: true,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn open_drawer(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let overlay = self.modal_overlay;

        window.open_drawer_at(Placement::Right, cx, move |this, _, cx| {
            this.overlay(overlay)
                .size(px(400.))
                .title("Add Connection")
                .gap_4()
                .child(format!("hi"))
                .footer(
                    h_flex()
                        .gap_6()
                        .items_center()
                        .child(Button::new("confirm").primary().label("Confirm").on_click(
                            |_, window, cx| {
                                window.close_drawer(cx);
                            },
                        ))
                        .child(
                            Button::new("cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| {
                                    window.close_drawer(cx);
                                }),
                        ),
                )
        });
    }

    // pub fn connect_to_database(
    //     &mut self,
    //     _: &ClickEvent,
    //     _window: &mut Window,
    //     cx: &mut Context<Self>,
    // ) {
    //     if self.is_loading {
    //         return;
    //     }

    //     self.is_loading = true;
    //     cx.notify();

    //     let db_manager = self.db_manager.clone();
    //     let connection_url = self.input_esc.read(cx).value().clone();

    //     cx.spawn(async move |this: WeakEntity<ConnectionsPanel>, cx| {
    //         let result = db_manager.connect(&connection_url).await;

    //         this.update(cx, |this, cx| {
    //             this.is_loading = false;
    //             match result {
    //                 Ok(_) => {
    //                     this.is_connected = true;
    //                     cx.emit(ConnectionEvent::Connected(this.db_manager.clone()));
    //                 }
    //                 Err(e) => {
    //                     let error_msg = format!("Failed to connect to database: {}", e);
    //                     eprintln!("{}", error_msg);
    //                     this.is_connected = false;
    //                     cx.emit(ConnectionEvent::ConnectionError { field1: error_msg });
    //                 }
    //             }
    //             cx.notify();
    //         })
    //         .ok();
    //     })
    //     .detach();
    // }

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
        let new_connection_button = Button::new("open-connection-form")
            .label("New Connection")
            .icon(IconName::Plus)
            .small()
            .on_click(cx.listener(Self::open_drawer));

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
                // .on_click(cx.listener(Self::connect_to_database))
        };

        v_flex()
            .gap_2()
            .p_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(new_connection_button)
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
            .bg(cx.theme().sidebar)
            .child(self.render_connections_list(cx))
            .child(self.render_connection_section(cx))

    }
}
