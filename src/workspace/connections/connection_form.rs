use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    form::{field, v_form},
    input::{Input, InputState},
    *,
};

use crate::{
    services::{ConnectionInfo, SslMode},
    state::{add_connection, connect, delete_connection, update_connection},
};

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
    active_connection: Option<ConnectionInfo>,
}

impl ConnectionForm {
    pub fn view(
        connection: Option<ConnectionInfo>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
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

            ConnectionForm {
                name,
                hostname,
                username,
                password,
                database,
                port,
                active_connection: connection,
            }
        })
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

        self.active_connection = None;

        cx.notify();
    }

    pub fn set_connection(
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
        self.active_connection = Some(connection.clone());
        cx.notify();
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(cx) {
            connect(&connection, cx);
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

        if self.active_connection.clone().is_some() {
            Some(ConnectionInfo {
                id: self.active_connection.clone().unwrap().id,
                name: name.to_string(),
                hostname: hostname.to_string(),
                username: username.to_string(),
                password: password.to_string(),
                database: database.to_string(),
                port: port_num,
                ssl_mode: SslMode::Prefer,
            })
        } else {
            Some(ConnectionInfo::new(
                name.to_string(),
                hostname.to_string(),
                username.to_string(),
                password.to_string(),
                database.to_string(),
                port_num,
                SslMode::Prefer,
            ))
        }
    }

    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(cx) {
            add_connection(connection, cx);
            self.clear(window, cx);
        }
    }

    fn update_connection(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(cx) {
            update_connection(connection, cx);
        }
    }

    fn delete_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(cx) {
            delete_connection(connection, cx);
            self.clear(window, cx);
        }
    }
}

impl Render for ConnectionForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .mb_4()
            .when(self.active_connection.clone().is_none(), |d| {
                d.child(div().text_3xl().child("Add Connection"))
            })
            .when(self.active_connection.clone().is_some(), |d| {
                d.child(div().text_3xl().child("Edit Connection"))
            })
            .child(
                v_form()
                    .columns(2)
                    .small()
                    .child(
                        field()
                            .col_span(2)
                            .label("Name")
                            .required(true)
                            .child(Input::new(&self.name)),
                    )
                    .child(
                        field()
                            .label("Host/Socket")
                            .required(true)
                            .child(Input::new(&self.hostname)),
                    )
                    .child(
                        field()
                            .label("Port")
                            .required(true)
                            .child(Input::new(&self.port)),
                    )
                    .child(
                        field()
                            .label("Username")
                            .col_span(2)
                            .required(true)
                            .child(Input::new(&self.username)),
                    )
                    .child(
                        field()
                            .col_span(2)
                            .label("Password")
                            .required(true)
                            .child(Input::new(&self.password)),
                    )
                    .child(
                        field()
                            .col_span(2)
                            .label("Database")
                            .required(true)
                            .child(Input::new(&self.database)),
                    )
                    .child(
                        field().label_indent(false).child(
                            h_flex()
                                .mt_2()
                                .gap_2()
                                .when(self.active_connection.clone().is_none(), |d| {
                                    d.child(
                                        Button::new("save-connection")
                                            .primary()
                                            .child("Save")
                                            .on_click(cx.listener(|this, _, win, cx| {
                                                this.save_connection(win, cx)
                                            })),
                                    )
                                })
                                .when(self.active_connection.clone().is_some(), |d| {
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
