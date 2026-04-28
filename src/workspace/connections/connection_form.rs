use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    form::{field, v_form},
    input::{Input, InputState},
    notification::NotificationType,
    select::{Select, SelectEvent, SelectState},
    switch::Switch,
    *,
};

use crate::{
    services::{
        ssh::{SshAuth, SshConfig},
        ConnectionInfo, ConnectionsRepository, DatabaseDriver, DatabaseManager, SslMode,
    },
    state::{add_connection, connect, delete_connection, update_connection},
};

#[allow(dead_code)]
pub enum ConnectionSavedEvent {
    ConnectionSaved,
    ConnectionSavedError { error: String },
}

impl EventEmitter<ConnectionSavedEvent> for ConnectionForm {}

/// Form for creating / editing a saved connection.
///
/// Layout:
/// 1. Driver selector (Postgres / MySQL) — toggling updates the default
///    port placeholder.
/// 2. Standard fields (name, host, port, user, password, database).
/// 3. Optional SSH tunnel section (toggle + host/port/user + auth).
pub struct ConnectionForm {
    name: Entity<InputState>,
    hostname: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    database: Entity<InputState>,
    port: Entity<InputState>,
    driver_select: Entity<SelectState<Vec<DatabaseDriver>>>,
    driver: DatabaseDriver,

    // SSH state
    ssh_enabled: bool,
    ssh_host: Entity<InputState>,
    ssh_port: Entity<InputState>,
    ssh_username: Entity<InputState>,
    ssh_auth_select: Entity<SelectState<Vec<SshAuthOption>>>,
    ssh_auth: SshAuth,
    ssh_key_path: Entity<InputState>,
    ssh_key_passphrase: Entity<InputState>,
    /// Set when editing an existing connection that already has a key
    /// passphrase stored in the keyring; in that case we don't require
    /// the user to re-enter it.
    ssh_passphrase_known: bool,

    active_connection: Option<ConnectionInfo>,
    is_testing: bool,
}

/// Wrapper so we can implement `SelectItem` for SSH auth choices without
/// touching the underlying `SshAuth` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshAuthOption {
    Agent,
    KeyFile,
}

impl SshAuthOption {
    fn label(&self) -> &'static str {
        match self {
            SshAuthOption::Agent => "SSH Agent",
            SshAuthOption::KeyFile => "Private Key File",
        }
    }

    fn all() -> Vec<SshAuthOption> {
        vec![SshAuthOption::Agent, SshAuthOption::KeyFile]
    }

    fn from_auth(auth: &SshAuth) -> Self {
        match auth {
            SshAuth::Agent => SshAuthOption::Agent,
            SshAuth::KeyFile { .. } => SshAuthOption::KeyFile,
        }
    }
}

impl gpui_component::select::SelectItem for SshAuthOption {
    type Value = &'static str;

    fn title(&self) -> SharedString {
        self.label().into()
    }

    fn value(&self) -> &Self::Value {
        match self {
            SshAuthOption::Agent => &"agent",
            SshAuthOption::KeyFile => &"key_file",
        }
    }
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

            // Driver selector
            let initial_driver = connection
                .as_ref()
                .map(|c| c.driver)
                .unwrap_or(DatabaseDriver::Postgres);
            let driver_select = cx.new(|cx| {
                SelectState::new(
                    DatabaseDriver::all(),
                    Some(IndexPath::new(initial_driver.to_index())),
                    window,
                    cx,
                )
            });
            cx.subscribe_in(&driver_select, window, Self::on_driver_change)
                .detach();

            // SSH inputs
            let ssh_host = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("ssh.example.com")
                    .clean_on_escape()
            });
            let ssh_port = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("22")
                    .clean_on_escape()
            });
            let ssh_username = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Username")
                    .clean_on_escape()
            });
            let ssh_key_path = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("/Users/you/.ssh/id_ed25519")
                    .clean_on_escape()
            });
            let ssh_key_passphrase = cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .placeholder("Passphrase (optional)")
                    .clean_on_escape()
            });

            let initial_ssh_auth = connection
                .as_ref()
                .and_then(|c| c.ssh.as_ref())
                .map(|s| SshAuthOption::from_auth(&s.auth))
                .unwrap_or(SshAuthOption::Agent);
            let ssh_auth_select = cx.new(|cx| {
                SelectState::new(
                    SshAuthOption::all(),
                    Some(IndexPath::new(match initial_ssh_auth {
                        SshAuthOption::Agent => 0,
                        SshAuthOption::KeyFile => 1,
                    })),
                    window,
                    cx,
                )
            });
            cx.subscribe_in(&ssh_auth_select, window, Self::on_ssh_auth_change)
                .detach();

            let ssh_enabled = connection.as_ref().and_then(|c| c.ssh.as_ref()).is_some();

            let ssh_auth = connection
                .as_ref()
                .and_then(|c| c.ssh.as_ref().map(|s| s.auth.clone()))
                .unwrap_or_default();

            let mut form = ConnectionForm {
                name,
                hostname,
                username,
                password,
                database,
                port,
                driver_select,
                driver: initial_driver,
                ssh_enabled,
                ssh_host,
                ssh_port,
                ssh_username,
                ssh_auth_select,
                ssh_auth,
                ssh_key_path,
                ssh_key_passphrase,
                ssh_passphrase_known: false,
                active_connection: connection.clone(),
                is_testing: false,
            };

            if let Some(c) = connection {
                form.populate_from(c, window, cx);
            } else {
                // New connection: set sensible default port placeholder.
                let default_port = initial_driver.default_port().to_string();
                let _ = form
                    .port
                    .update(cx, |this, cx| this.set_value(default_port, window, cx));
            }
            form
        })
    }

    fn on_driver_change(
        &mut self,
        _: &Entity<SelectState<Vec<DatabaseDriver>>>,
        event: &SelectEvent<Vec<DatabaseDriver>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let SelectEvent::Confirm(Some(value)) = event {
            let new_driver = match *value {
                "mysql" => DatabaseDriver::MySql,
                _ => DatabaseDriver::Postgres,
            };
            let prev = self.driver;
            self.driver = new_driver;
            // If the user hadn't changed the port from the previous default,
            // swap it to the new driver's default. Otherwise leave it alone.
            let prev_default = prev.default_port().to_string();
            let current = self.port.read(cx).value().to_string();
            if current.is_empty() || current == prev_default {
                let new_default = new_driver.default_port().to_string();
                let _ = self
                    .port
                    .update(cx, |this, cx| this.set_value(new_default, window, cx));
            }
            cx.notify();
        }
    }

    fn on_ssh_auth_change(
        &mut self,
        _: &Entity<SelectState<Vec<SshAuthOption>>>,
        event: &SelectEvent<Vec<SshAuthOption>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let SelectEvent::Confirm(Some(value)) = event {
            self.ssh_auth = match *value {
                "key_file" => SshAuth::KeyFile {
                    path: self.ssh_key_path.read(cx).value().to_string(),
                },
                _ => SshAuth::Agent,
            };
            cx.notify();
        }
    }

    fn populate_from(
        &mut self,
        connection: ConnectionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self
            .name
            .update(cx, |this, cx| this.set_value(connection.name.clone(), window, cx));
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

        if let Some(ssh) = &connection.ssh {
            self.ssh_enabled = true;
            let _ = self.ssh_host.update(cx, |this, cx| {
                this.set_value(ssh.host.clone(), window, cx)
            });
            let _ = self.ssh_port.update(cx, |this, cx| {
                this.set_value(ssh.port.to_string(), window, cx)
            });
            let _ = self.ssh_username.update(cx, |this, cx| {
                this.set_value(ssh.username.clone(), window, cx)
            });
            self.ssh_auth = ssh.auth.clone();
            if let SshAuth::KeyFile { path } = &ssh.auth {
                let _ = self.ssh_key_path.update(cx, |this, cx| {
                    this.set_value(path.clone(), window, cx)
                });
            }
            self.ssh_passphrase_known =
                ConnectionsRepository::get_ssh_key_passphrase(&connection.id).is_some();
        }
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in [
            &self.name,
            &self.hostname,
            &self.username,
            &self.password,
            &self.database,
            &self.port,
            &self.ssh_host,
            &self.ssh_port,
            &self.ssh_username,
            &self.ssh_key_path,
            &self.ssh_key_passphrase,
        ] {
            let _ = input.update(cx, |this, cx| this.set_value("", window, cx));
        }
        self.ssh_enabled = false;
        self.ssh_auth = SshAuth::Agent;
        self.ssh_passphrase_known = false;
        self.active_connection = None;
        cx.notify();
    }

    pub fn set_connection(
        &mut self,
        connection: ConnectionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Reset SSH state and update driver selector to match the loaded
        // connection before populating fields.
        self.driver = connection.driver;
        let driver_index = connection.driver.to_index();
        self.driver_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(driver_index)), window, cx);
        });
        self.ssh_enabled = false;
        self.ssh_auth = SshAuth::Agent;
        self.ssh_passphrase_known = false;
        self.active_connection = Some(connection.clone());
        self.populate_from(connection, window, cx);
        cx.notify();
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(window, cx) {
            // Persist any SSH key passphrase the user typed (when applicable).
            self.persist_ssh_passphrase_if_needed(&connection, cx);
            connect(&connection, cx);
            self.clear(window, cx);
            cx.notify();
        }
    }

    fn build_ssh_config(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<SshConfig> {
        if !self.ssh_enabled {
            return None;
        }

        let host = self.ssh_host.read(cx).value().to_string();
        let user = self.ssh_username.read(cx).value().to_string();
        let port_str = self.ssh_port.read(cx).value().to_string();

        if host.is_empty() || user.is_empty() {
            window.push_notification(
                (
                    NotificationType::Error,
                    "SSH host and username are required when SSH is enabled.",
                ),
                cx,
            );
            return None;
        }

        let port: u16 = if port_str.is_empty() {
            22
        } else {
            match port_str.parse() {
                Ok(p) if (1..=65535).contains(&p) => p,
                _ => {
                    window.push_notification(
                        (NotificationType::Error, "Invalid SSH port."),
                        cx,
                    );
                    return None;
                }
            }
        };

        let auth = match self.ssh_auth.clone() {
            SshAuth::Agent => SshAuth::Agent,
            SshAuth::KeyFile { .. } => {
                let path = self.ssh_key_path.read(cx).value().to_string();
                if path.is_empty() {
                    window.push_notification(
                        (
                            NotificationType::Error,
                            "Private key path is required for key-file authentication.",
                        ),
                        cx,
                    );
                    return None;
                }
                SshAuth::KeyFile { path }
            }
        };

        Some(SshConfig {
            host,
            port,
            username: user,
            auth,
        })
    }

    /// If an SSH config with key-file auth was provided and the user typed
    /// a fresh passphrase, persist it to the keyring so reconnects work.
    fn persist_ssh_passphrase_if_needed(
        &mut self,
        connection: &ConnectionInfo,
        cx: &mut Context<Self>,
    ) {
        if let Some(SshConfig {
            auth: SshAuth::KeyFile { .. },
            ..
        }) = &connection.ssh
        {
            let passphrase = self.ssh_key_passphrase.read(cx).value().to_string();
            if !passphrase.is_empty() {
                if let Err(e) = ConnectionsRepository::store_ssh_key_passphrase(
                    &connection.id,
                    &passphrase,
                ) {
                    tracing::warn!("Failed to store SSH key passphrase: {}", e);
                }
            }
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

        let port_num: usize = match port.parse() {
            Ok(n) if (1..=65_535).contains(&n) => n,
            _ => {
                window.push_notification((NotificationType::Error, "Invalid port number."), cx);
                return None;
            }
        };

        let ssh = self.build_ssh_config(window, cx);
        // build_ssh_config returns None either because SSH is off or
        // because validation failed and a notification was emitted.
        if self.ssh_enabled && ssh.is_none() {
            return None;
        }

        let id = self
            .active_connection
            .as_ref()
            .map(|c| c.id)
            .unwrap_or_else(uuid::Uuid::new_v4);

        Some(ConnectionInfo {
            id,
            name: name.to_string(),
            driver: self.driver,
            hostname: hostname.to_string(),
            username: username.to_string(),
            password,
            database: database.to_string(),
            port: port_num,
            ssl_mode: SslMode::Prefer,
            ssh,
        })
    }

    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(window, cx) {
            self.persist_ssh_passphrase_if_needed(&connection, cx);
            add_connection(connection, cx);
            self.clear(window, cx);
        }
    }

    fn update_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(window, cx) {
            self.persist_ssh_passphrase_if_needed(&connection, cx);
            update_connection(connection, cx);
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

            // Persist before testing so the SSH key passphrase, if any,
            // is available to the tunnel via the keyring.
            self.persist_ssh_passphrase_if_needed(&connection, cx);

            let entity = cx.entity();
            let conn_for_test = connection.clone();

            cx.spawn_in(window, async move |_this, cx| {
                let result = DatabaseManager::test_connection(&conn_for_test).await;

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

    fn render_ssh_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_key_fields = matches!(self.ssh_auth, SshAuth::KeyFile { .. });
        let passphrase_hint: Option<SharedString> = if self.ssh_passphrase_known {
            Some("Saved passphrase will be used; type to override.".into())
        } else {
            None
        };

        v_form()
            .columns(2)
            .small()
            .child(
                field()
                    .col_span(2)
                    .label_indent(false)
                    .child(
                        Switch::new("ssh-enabled")
                            .checked(self.ssh_enabled)
                            .label("Connect through SSH tunnel")
                            .on_click(cx.listener(|this, checked: &bool, _win, cx| {
                                this.ssh_enabled = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.ssh_enabled, |f| {
                f.child(
                    field()
                        .label("SSH Host")
                        .required(true)
                        .child(Input::new(&self.ssh_host)),
                )
                .child(
                    field()
                        .label("SSH Port")
                        .child(Input::new(&self.ssh_port)),
                )
                .child(
                    field()
                        .col_span(2)
                        .label("SSH User")
                        .required(true)
                        .child(Input::new(&self.ssh_username)),
                )
                .child(
                    field()
                        .col_span(2)
                        .label("SSH Auth")
                        .child(Select::new(&self.ssh_auth_select)),
                )
                .when(show_key_fields, |inner| {
                    let mut inner = inner
                        .child(
                            field()
                                .col_span(2)
                                .label("Private Key Path")
                                .required(true)
                                .child(Input::new(&self.ssh_key_path)),
                        )
                        .child(
                            field()
                                .col_span(2)
                                .label("Key Passphrase")
                                .child(Input::new(&self.ssh_key_passphrase)),
                        );
                    if let Some(hint) = passphrase_hint.clone() {
                        inner = inner.child(
                            field().col_span(2).label_indent(false).child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(hint),
                            ),
                        );
                    }
                    inner
                })
            })
    }
}

impl Render for ConnectionForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_edit = self.active_connection.is_some();
        let driver_label: SharedString = self.driver.as_str().into();

        div()
            .mb_4()
            .when(!is_edit, |d| d.child(div().text_3xl().child("Add Connection")))
            .when(is_edit, |d| d.child(div().text_3xl().child("Edit Connection")))
            .child(
                v_form()
                    .columns(2)
                    .small()
                    .child(
                        field()
                            .col_span(2)
                            .label("Driver")
                            .required(true)
                            .child(Select::new(&self.driver_select)),
                    )
                    .child(
                        field()
                            .col_span(2)
                            .label("Name")
                            .required(true)
                            .child(Input::new(&self.name)),
                    )
                    .child(
                        field()
                            .label("Host")
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
            .child(
                div()
                    .mt_4()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Selected driver: {}", driver_label)),
            )
            .child(div().mt_2().child(self.render_ssh_section(cx)))
            .child(
                div().mt_4().child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("test-connection")
                                .child("Test Connection")
                                .loading(self.is_testing)
                                .on_click(cx.listener(|this, _, win, cx| {
                                    this.test_connection(win, cx)
                                })),
                        )
                        .when(!is_edit, |d| {
                            d.child(
                                Button::new("save-connection")
                                    .primary()
                                    .child("Save")
                                    .on_click(cx.listener(|this, _, win, cx| {
                                        this.save_connection(win, cx)
                                    })),
                            )
                        })
                        .when(is_edit, |d| {
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
