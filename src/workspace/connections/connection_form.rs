use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    form::{field, v_form},
    input::{Input, InputState},
    label::Label,
    notification::NotificationType,
    select::{Select, SelectState},
    switch::Switch,
    *,
};

use crate::{
    services::{
        ConnectionInfo, ConnectionsRepository, DatabaseManager, SshAuthType, SshService,
        SshTunnelInfo, SslMode,
    },
    state::{add_connection, connect, delete_connection, update_connection},
};

#[allow(dead_code)]
pub enum ConnectionSavedEvent {
    ConnectionSaved,
    ConnectionSavedError { error: String },
}

impl EventEmitter<ConnectionSavedEvent> for ConnectionForm {}

pub struct ConnectionForm {
    // Database fields
    name: Entity<InputState>,
    hostname: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    database: Entity<InputState>,
    port: Entity<InputState>,

    // SSH tunnel fields
    ssh_enabled: bool,
    ssh_host: Entity<InputState>,
    ssh_port: Entity<InputState>,
    ssh_user: Entity<InputState>,
    ssh_auth_type: Entity<SelectState<Vec<SharedString>>>,
    ssh_password: Entity<InputState>,
    ssh_private_key_path: Entity<InputState>,

    active_connection: Option<ConnectionInfo>,
    is_testing: bool,
}

impl ConnectionForm {
    pub fn view(
        connection: Option<ConnectionInfo>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            // Database input fields
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

            // SSH tunnel input fields
            let ssh_host = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("SSH Host")
                    .clean_on_escape()
            });
            let ssh_port = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("22")
                    .clean_on_escape()
            });
            let ssh_user = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("SSH Username")
                    .clean_on_escape()
            });
            let ssh_password = cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .placeholder("SSH Password")
                    .clean_on_escape()
            });
            let ssh_private_key_path = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("~/.ssh/id_rsa")
                    .clean_on_escape()
            });

            // SSH auth type selector
            let auth_types: Vec<SharedString> = SshAuthType::all()
                .iter()
                .map(|t| SharedString::from(t.as_str()))
                .collect();
            let ssh_auth_type =
                cx.new(|cx| SelectState::new(auth_types, Some(IndexPath::new(0)), window, cx));

            ConnectionForm {
                name,
                hostname,
                username,
                password,
                database,
                port,
                ssh_enabled: false,
                ssh_host,
                ssh_port,
                ssh_user,
                ssh_password,
                ssh_auth_type,
                ssh_private_key_path,
                active_connection: connection,
                is_testing: false,
            }
        })
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Clear database fields
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

        // Clear SSH fields
        self.ssh_enabled = false;
        let _ = self
            .ssh_host
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .ssh_port
            .update(cx, |this, cx| this.set_value("22", window, cx));
        let _ = self
            .ssh_user
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .ssh_private_key_path
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self.ssh_auth_type.update(cx, |this, cx| {
            this.set_selected_index(Some(IndexPath::default()), window, cx);
        });

        self.active_connection = None;
        cx.notify();
    }

    pub fn set_connection(
        &mut self,
        connection: ConnectionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Set database fields
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

        // Set SSH tunnel fields
        if let Some(ref ssh) = connection.ssh_tunnel {
            self.ssh_enabled = ssh.enabled;
            let _ = self.ssh_host.update(cx, |this, cx| {
                this.set_value(ssh.ssh_host.clone(), window, cx)
            });
            let _ = self.ssh_port.update(cx, |this, cx| {
                this.set_value(ssh.ssh_port.to_string(), window, cx)
            });
            let _ = self.ssh_user.update(cx, |this, cx| {
                this.set_value(ssh.ssh_user.clone(), window, cx)
            });
            let _ = self.ssh_auth_type.update(cx, |this, cx| {
                this.set_selected_index(Some(IndexPath::new(ssh.auth_type.to_index())), window, cx);
            });
            if let Some(ref key_path) = ssh.private_key_path {
                let _ = self
                    .ssh_private_key_path
                    .update(cx, |this, cx| this.set_value(key_path.clone(), window, cx));
            }
            // Load SSH password from keychain if using password auth
            if ssh.auth_type == SshAuthType::Password {
                if let Some(stored_password) =
                    SshService::get_stored_password(&ssh.ssh_host, ssh.ssh_port, &ssh.ssh_user)
                {
                    let _ = self
                        .ssh_password
                        .update(cx, |this, cx| this.set_value(stored_password, window, cx));
                }
            }
        } else {
            self.ssh_enabled = false;
            let _ = self
                .ssh_host
                .update(cx, |this, cx| this.set_value("", window, cx));
            let _ = self
                .ssh_port
                .update(cx, |this, cx| this.set_value("22", window, cx));
            let _ = self
                .ssh_user
                .update(cx, |this, cx| this.set_value("", window, cx));
            let _ = self
                .ssh_private_key_path
                .update(cx, |this, cx| this.set_value("", window, cx));
        }

        self.active_connection = Some(connection.clone());
        cx.notify();
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(window, cx) {
            // Store SSH password in keychain if using password auth
            self.store_ssh_password_if_needed(&connection, cx);
            connect(&connection, cx);
            self.clear(window, cx);
            cx.notify();
        }
    }

    fn get_connection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ConnectionInfo> {
        let name = self.name.read(cx).value();
        let hostname = self.hostname.read(cx).value();
        let username = self.username.read(cx).value();
        let password = self.password.read(cx).value();
        let database = self.database.read(cx).value();
        let port = self.port.read(cx).value();

        // For editing: if password is empty, try to fetch from keychain
        let password = if password.is_empty() {
            if let Some(ref active) = self.active_connection {
                ConnectionsRepository::get_connection_password(&active.id).unwrap_or_default()
            } else {
                password.to_string()
            }
        } else {
            password.to_string()
        };

        // Validate database inputs
        if name.is_empty()
            || hostname.is_empty()
            || username.is_empty()
            || password.is_empty()
            || database.is_empty()
            || port.is_empty()
        {
            window.push_notification(
                (
                    NotificationType::Error,
                    "Not all fields have values. Please try again.",
                ),
                cx,
            );
            return None;
        }

        let port_num = match port.parse::<usize>() {
            Ok(num) => num,
            Err(e) => {
                tracing::error!("Failed to parse port: {}", e);
                window.push_notification((NotificationType::Error, "Invalid port number."), cx);
                return None;
            }
        };

        if port_num < 1 || port_num > 65_535 {
            window.push_notification((NotificationType::Error, "Invalid port number."), cx);
            return None;
        }

        // Build SSH tunnel info if enabled
        let ssh_tunnel = if self.ssh_enabled {
            let ssh_host = self.ssh_host.read(cx).value();
            let ssh_port_str = self.ssh_port.read(cx).value();
            let ssh_user = self.ssh_user.read(cx).value();

            // Validate SSH inputs
            if ssh_host.is_empty() || ssh_user.is_empty() {
                window.push_notification(
                    (
                        NotificationType::Error,
                        "SSH Host and Username are required when SSH is enabled.",
                    ),
                    cx,
                );
                return None;
            }

            let ssh_port: u16 = ssh_port_str.parse().unwrap_or(22);

            let auth_type_index = self
                .ssh_auth_type
                .read(cx)
                .selected_index(cx)
                .map(|i| i.row) // verify
                .unwrap_or(0);
            let auth_type = SshAuthType::from_index(auth_type_index);

            let private_key_path = if auth_type == SshAuthType::PublicKey {
                let path = self.ssh_private_key_path.read(cx).value();
                if path.is_empty() {
                    None
                } else {
                    Some(path.to_string())
                }
            } else {
                None
            };

            Some(SshTunnelInfo {
                enabled: true,
                ssh_host: ssh_host.to_string(),
                ssh_port,
                ssh_user: ssh_user.to_string(),
                auth_type,
                private_key_path,
            })
        } else {
            None
        };

        let connection = if self.active_connection.clone().is_some() {
            ConnectionInfo {
                id: self.active_connection.clone().unwrap().id,
                name: name.to_string(),
                hostname: hostname.to_string(),
                username: username.to_string(),
                password: password.to_string(),
                database: database.to_string(),
                port: port_num,
                ssl_mode: SslMode::Prefer,
                ssh_tunnel,
            }
        } else {
            ConnectionInfo {
                id: uuid::Uuid::new_v4(),
                name: name.to_string(),
                hostname: hostname.to_string(),
                username: username.to_string(),
                password: password.to_string(),
                database: database.to_string(),
                port: port_num,
                ssl_mode: SslMode::Prefer,
                ssh_tunnel,
            }
        };

        Some(connection)
    }

    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(window, cx) {
            // Store SSH password in keychain if using password auth
            self.store_ssh_password_if_needed(&connection, cx);
            add_connection(connection, cx);
            self.clear(window, cx);
        }
    }

    fn update_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(window, cx) {
            // Store SSH password in keychain if using password auth
            self.store_ssh_password_if_needed(&connection, cx);
            update_connection(connection, cx);
        }
    }

    /// Helper to store SSH password in keychain
    fn store_ssh_password_if_needed(&self, connection: &ConnectionInfo, cx: &Context<Self>) {
        if let Some(ref ssh) = connection.ssh_tunnel {
            if ssh.enabled && ssh.auth_type == SshAuthType::Password {
                let ssh_password = self.ssh_password.read(cx).value();
                if !ssh_password.is_empty() {
                    if let Err(e) = SshService::store_password(
                        &ssh.ssh_host,
                        ssh.ssh_port,
                        &ssh.ssh_user,
                        &ssh_password,
                    ) {
                        tracing::error!("Failed to store SSH password: {}", e);
                    }
                }
            }
        }
    }

    fn delete_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.active_connection.clone() {
            delete_connection(connection, cx);
            self.clear(window, cx);
        }
    }

    fn test_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_testing {
            return;
        }

        if let Some(connection) = self.get_connection(window, cx) {
            self.is_testing = true;
            cx.notify();

            // TODO: For SSH tunnel connections, we should test the full path
            // For now, just test direct connection
            let connect_options = connection.to_pg_connect_options();
            let entity = cx.entity();

            cx.spawn_in(window, async move |_this, cx| {
                let result = DatabaseManager::test_connection_options(connect_options).await;

                let _ = cx.update(|window, cx| {
                    match result {
                        Ok(_) => {
                            window.push_notification(
                                (NotificationType::Success, "Connection successful!"),
                                cx,
                            );
                        }
                        Err(e) => {
                            let error_msg: SharedString =
                                format!("Connection failed: {}", e).into();
                            tracing::error!("{}", error_msg.clone());
                            window.push_notification((NotificationType::Error, error_msg), cx);
                        }
                    }

                    cx.update_entity(&entity, |form, cx| {
                        form.is_testing = false;
                        cx.notify();
                    });
                });
            })
            .detach();
        }
    }

    fn toggle_ssh(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.ssh_enabled = !self.ssh_enabled;
        cx.notify();
    }

    fn render_ssh_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ssh_enabled = self.ssh_enabled;
        let auth_type_index = self
            .ssh_auth_type
            .read(cx)
            .selected_index(cx)
            .map(|i| i.row) // verify
            .unwrap_or(0);
        let show_key_path = auth_type_index == 2; // PublicKey

        div()
            .mt_4()
            .pt_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mb_2()
                    .child(
                        Switch::new("ssh_enabled")
                            .checked(ssh_enabled)
                            .on_click(cx.listener(|this, _, win, cx| {
                                this.toggle_ssh(win, cx);
                            })),
                    )
                    .child(Label::new("Connect via SSH Tunnel")),
            )
            .when(ssh_enabled, |this| {
                this.child(
                    v_form()
                        .columns(2)
                        .small()
                        .child(
                            field()
                                .label("SSH Host")
                                .required(true)
                                .child(Input::new(&self.ssh_host)),
                        )
                        .child(
                            field()
                                .label("SSH Port")
                                .required(true)
                                .child(Input::new(&self.ssh_port)),
                        )
                        .child(
                            field()
                                .label("SSH Username")
                                .col_span(2)
                                .required(true)
                                .child(Input::new(&self.ssh_user)),
                        )
                        .child(
                            field().label("Auth Method").col_span(2).child(
                                Select::new(&self.ssh_auth_type)
                                    .cleanable(false)
                                    .menu_width(px(200.)),
                            ),
                        )
                        .when(show_key_path, |form| {
                            form.child(
                                field()
                                    .label("Private Key Path")
                                    .col_span(2)
                                    .child(Input::new(&self.ssh_private_key_path)),
                            )
                        })
                        .when(auth_type_index == 1, |form| {
                            // 1 = Password
                            form.child(
                                field()
                                    .label("SSH Password")
                                    .col_span(2)
                                    .child(Input::new(&self.ssh_password)),
                            )
                        }),
                )
            })
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
                    ),
            )
            // SSH Tunnel Section
            .child(self.render_ssh_section(cx))
            // Action Buttons
            .child(
                field().label_indent(false).child(
                    h_flex()
                        .mt_4()
                        .gap_2()
                        .child(
                            Button::new("test-connection")
                                .child("Test Connection")
                                .loading(self.is_testing)
                                .on_click(
                                    cx.listener(|this, _, win, cx| this.test_connection(win, cx)),
                                ),
                        )
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
                                    .on_click(cx.listener(|_this, _, win, cx| {
                                        let entity = cx.entity();
                                        win.open_dialog(cx, move |dialog, _win, _cx| {
                                            let entity_clone = entity.clone();
                                            dialog
                                            .confirm()
                                            .child(
                                                "Are you sure you want to delete this connection?",
                                            )
                                            .on_ok(move |_, window, cx| {
                                                cx.update_entity(
                                                    &entity_clone.clone(),
                                                    |entity, cx| {
                                                        entity.delete_connection(window, cx);
                                                        cx.notify();
                                                    },
                                                );
                                                window.push_notification(
                                                    (NotificationType::Success, "Deleted"),
                                                    cx,
                                                );
                                                true
                                            })
                                        });
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
            )
            .text_sm()
    }
}
