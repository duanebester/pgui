use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    form::{form_field, v_form},
    input::{InputState, TextInput},
    *,
};

use crate::{services::ConnectionInfo, state::ConnectionState};

#[allow(dead_code)]
pub enum ConnectionSavedEvent {
    ConnectionSaved,
    ConnectionSavedError { error: String },
}

impl EventEmitter<ConnectionSavedEvent> for ConnectionForm {}

pub struct ConnectionForm {
    name: Entity<InputState>,
    hostname: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    database: Entity<InputState>,
    port: Entity<InputState>,
    is_editing: bool,
    active_connection: Option<ConnectionInfo>,
    _subscriptions: Vec<Subscription>,
}

impl ConnectionForm {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let name = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Name")
                    .clean_on_escape()
            });
            let hostname = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Hostname")
                    .clean_on_escape()
            });
            let username = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Username")
                    .clean_on_escape()
            });
            let password = cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .placeholder("Password")
                    .clean_on_escape()
            });
            let database = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Database")
                    .clean_on_escape()
            });
            let port = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Port")
                    .clean_on_escape()
            });

            let _subscriptions = vec![cx.observe_global_in::<ConnectionState>(
                window,
                |this: &mut ConnectionForm, win, cx| {
                    let active_connection =
                        cx.global::<ConnectionState>().active_connection.clone();
                    if let Some(conn) = active_connection {
                        this.is_editing = true;
                        this.active_connection = Some(conn.clone());
                        this.set_connection(conn, win, cx);
                        cx.notify();
                    }
                },
            )];

            ConnectionForm {
                name,
                hostname,
                username,
                password,
                database,
                port,
                is_editing: false,
                active_connection: None,
                _subscriptions,
            }
        })
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self
            .name
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .hostname
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .username
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .password
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .database
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .port
            .update(cx, |this, cx| this.set_value("", window, cx));

        self.is_editing = false;
        cx.notify();
    }

    fn set_connection(
        &mut self,
        connection: ConnectionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.name.update(cx, |this, cx| {
            this.set_value(connection.name.clone(), window, cx)
        });
        let _ = self.hostname.update(cx, |this, cx| {
            this.set_value(connection.hostname.clone(), window, cx)
        });
        let _ = self.username.update(cx, |this, cx| {
            this.set_value(connection.username.clone(), window, cx)
        });
        let _ = self.password.update(cx, |this, cx| {
            this.set_value(connection.password.clone(), window, cx)
        });
        let _ = self.database.update(cx, |this, cx| {
            this.set_value(connection.database.clone(), window, cx)
        });
        let _ = self.port.update(cx, |this, cx| {
            this.set_value(connection.port.to_string(), window, cx)
        });
        cx.notify();
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(cx) {
            ConnectionState::connect(&connection, cx);
            self.clear(window, cx);
            cx.notify();
        }
    }

    fn get_connection(&mut self, cx: &mut Context<Self>) -> Option<ConnectionInfo> {
        let name = self.name.read(cx).value();
        let hostname = self.hostname.read(cx).value();
        let username = self.username.read(cx).value();
        let password = self.password.read(cx).value();
        let database = self.database.read(cx).value();
        let port = self.port.read(cx).value();

        // Validate inputs
        if name.is_empty()
            || hostname.is_empty()
            || username.is_empty()
            || password.is_empty()
            || database.is_empty()
            || port.is_empty()
        {
            // TODO: Show validation error
            return None;
        }

        let port_num = match port.parse::<usize>() {
            Ok(num) => {
                println!("Successfully parsed: {}", num);
                num // The result of the match expression is the parsed number
            }
            Err(e) => {
                eprintln!("Failed to parse integer: {}", e);
                // Return a default value or handle the error in another way
                0
            }
        };

        if port_num < 1 {
            // TODO: Show validation error
            return None;
        }

        if port_num > 65_535 {
            // TODO: Show validation error
            return None;
        }

        if self.is_editing && self.active_connection.clone().is_some() {
            Some(ConnectionInfo {
                id: self.active_connection.clone().unwrap().id,
                name: name.to_string(),
                hostname: hostname.to_string(),
                username: username.to_string(),
                password: password.to_string(),
                database: database.to_string(),
                port: port_num,
            })
        } else {
            Some(ConnectionInfo::new(
                name.to_string(),
                hostname.to_string(),
                username.to_string(),
                password.to_string(),
                database.to_string(),
                port_num,
            ))
        }
    }

    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(cx) {
            ConnectionState::add_connection(connection, cx);
            self.clear(window, cx);
        }
    }

    fn update_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(cx) {
            ConnectionState::update_connection(connection, cx);
            self.clear(window, cx);
        }
    }

    fn delete_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(cx) {
            ConnectionState::delete_connection(connection, cx);
            self.clear(window, cx);
        }
    }
}

impl Render for ConnectionForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .mb_4()
            .when(!self.is_editing, |d| {
                d.child(div().text_3xl().child("Add Connection"))
            })
            .when(self.is_editing, |d| {
                d.child(div().text_3xl().child("Edit Connection"))
            })
            .child(
                v_form()
                    .column(2)
                    .small()
                    .child(
                        form_field()
                            .col_span(2)
                            .label("Name")
                            .required(true)
                            .child(TextInput::new(&self.name)),
                    )
                    .child(
                        form_field()
                            .label("Host/Socket")
                            .required(true)
                            .child(TextInput::new(&self.hostname)),
                    )
                    .child(
                        form_field()
                            .label("Port")
                            .required(true)
                            .child(TextInput::new(&self.port)),
                    )
                    .child(
                        form_field()
                            .label("Username")
                            .col_span(2)
                            .required(true)
                            .child(TextInput::new(&self.username)),
                    )
                    .child(
                        form_field()
                            .col_span(2)
                            .label("Password")
                            .required(true)
                            .child(TextInput::new(&self.password)),
                    )
                    .child(
                        form_field()
                            .col_span(2)
                            .label("Database")
                            .required(true)
                            .child(TextInput::new(&self.database)),
                    )
                    .child(
                        form_field().no_label_indent().child(
                            h_flex()
                                .mt_2()
                                .gap_2()
                                .child(
                                    Button::new("clear-connection").child("Clear").on_click(
                                        cx.listener(|this, _, win, cx| this.clear(win, cx)),
                                    ),
                                )
                                .when(!self.is_editing, |d| {
                                    d.child(
                                        Button::new("save-connection")
                                            .primary()
                                            .child("Save")
                                            .on_click(cx.listener(|this, _, win, cx| {
                                                this.save_connection(win, cx)
                                            })),
                                    )
                                })
                                .when(self.is_editing, |d| {
                                    d.child(
                                        Button::new("delete-connection")
                                            .child("Delete")
                                            .danger()
                                            .on_click(cx.listener(|this, _, win, cx| {
                                                this.delete_connection(win, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("update-connection")
                                            .primary()
                                            .child("Update")
                                            .on_click(cx.listener(|this, _, win, cx| {
                                                this.update_connection(win, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("connect").primary().child("Connect").on_click(
                                            cx.listener(|this, _, win, cx| this.connect(win, cx)),
                                        ),
                                    )
                                }),
                        ),
                    ),
            )
            .text_sm()
    }
}
