use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt,
    button::{Button, ButtonVariants as _},
    label::Label,
    list::{List, ListEvent, ListState},
    v_flex,
};

use crate::{
    services::ConnectionInfo,
    state::ConnectionState,
    workspace::connections::{ConnectionForm, ConnectionListDelegate},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct ConnectionManager {
    is_creating: bool,
    is_editing: bool,
    selected_connection: Option<ConnectionInfo>,
    connection_form: Entity<ConnectionForm>,
    connection_list: Entity<ListState<ConnectionListDelegate>>,
    _subscriptions: Vec<Subscription>,
}

impl ConnectionManager {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let connection_list =
            cx.new(|cx| ListState::new(ConnectionListDelegate::new(), window, cx));

        let conn_list_clone = connection_list.clone();
        let _subscriptions = vec![
            cx.observe_global::<ConnectionState>(move |_this, cx| {
                let conns = cx.global::<ConnectionState>().saved_connections.clone();
                let _ = cx.update_entity(&conn_list_clone, |list, cx| {
                    list.delegate_mut().update_connections(conns);
                    cx.notify();
                });

                cx.notify();
            }),
            cx.subscribe_in(
                &connection_list.clone(),
                window,
                |this, list, evt, win, cx| {
                    match evt.clone() {
                        ListEvent::Confirm(ix) => {
                            let list_del = list.read(cx).delegate();
                            if let Some(conn) = list_del.matched_connections.clone().get(ix.row) {
                                this.selected_connection = Some(conn.clone());
                                this.is_creating = false;
                                this.is_editing = false;
                                cx.notify();

                                let _ =
                                    cx.update_entity(&this.connection_form.clone(), |form, cx| {
                                        form.set_connection(conn.clone(), win, cx);
                                        cx.notify();
                                    });
                            }
                        }
                        _ => {
                            println!("not confirm")
                        }
                    };

                    // if let Some(c) = connection {
                    //     let _ = cx.update_entity(&this.connection_form.clone(), |form, cx| {
                    //         form.set_connection(c, win, cx);
                    //         cx.notify();
                    //     });
                    // }
                },
            ),
        ];

        let connection_form = ConnectionForm::view(None, window, cx);

        Self {
            is_creating: false,
            is_editing: false,
            selected_connection: None,
            connection_form,
            connection_list,
            _subscriptions,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn render_connections_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = div()
            .pl_1()
            .flex()
            .flex_row()
            .w_full()
            .justify_between()
            .items_center()
            .child(Label::new("Connections").font_bold().text_base())
            .child(
                Button::new("new")
                    .icon(Icon::empty().path("icons/plus.svg"))
                    .tooltip("New Connection")
                    .ghost()
                    .small()
                    .on_click(cx.listener(|this, _evt, win, cx| {
                        this.is_creating = true;
                        this.is_editing = false;
                        this.selected_connection = None;
                        cx.update_entity(&this.connection_form, |form, cx| {
                            form.clear(win, cx);
                            cx.notify();
                        });
                        cx.notify();
                    })),
            );
        v_flex()
            .gap_2()
            .p_2()
            .flex_1()
            .items_start()
            .child(title)
            .child(
                List::new(&self.connection_list)
                    .p(px(8.))
                    .flex_1()
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius),
            )
    }
}

impl Render for ConnectionManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = div()
            .id("connection-manager-sidebar")
            .flex()
            .h_full()
            .bg(cx.theme().sidebar)
            .border_color(cx.theme().border)
            .border_r_1()
            .min_w(px(300.0))
            .child(self.render_connections_list(cx));

        let show_wecome = self.selected_connection.clone().is_none()
            && !self.is_creating.clone()
            && !self.is_editing.clone();

        let show_connection_info = self.selected_connection.clone().is_some()
            && !self.is_creating.clone()
            && !self.is_editing.clone();

        let show_form = self.is_editing.clone() || self.is_creating.clone();

        let main = div()
            .id("connection-manager-main")
            .flex()
            .bg(cx.theme().tiles)
            .flex_col()
            .w_full()
            .p_4()
            .when(show_connection_info, |d| {
                let conn = self.selected_connection.clone().unwrap();
                d.flex().justify_center().items_center().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .items_center()
                        .child(div().text_xl().child(conn.name.clone()))
                        .child(div().text_lg().child(format!(
                            "{}@{}:{}/{}",
                            conn.username.clone(),
                            conn.hostname.clone(),
                            conn.port.clone(),
                            conn.database.clone()
                        )))
                        .child(
                            div()
                                .flex()
                                .justify_center()
                                .gap_1()
                                .child(
                                    Button::new("delete")
                                        .label("Delete")
                                        .icon(Icon::empty().path("icons/trash.svg"))
                                        .tooltip("Delete")
                                        .ghost()
                                        .small()
                                        .on_click(cx.listener(|this, _evt, win, cx| {
                                            this.is_creating = false;
                                            this.is_editing = false;

                                            cx.update_entity(&this.connection_form, |form, cx| {
                                                form.clear(win, cx);
                                                cx.notify();
                                            });

                                            if let Some(conn) = this.selected_connection.clone() {
                                                ConnectionState::delete_connection(conn, cx);
                                            }

                                            this.selected_connection = None;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("edit")
                                        .label("Edit")
                                        .icon(Icon::empty().path("icons/pencil-line.svg"))
                                        .tooltip("Edit")
                                        .ghost()
                                        .small()
                                        .on_click(cx.listener(|this, _evt, _win, cx| {
                                            this.is_editing = true;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("connect")
                                        .label("Connect")
                                        .icon(Icon::empty().path("icons/cable.svg"))
                                        .tooltip("Connect")
                                        .ghost()
                                        .small()
                                        .on_click(cx.listener(|this, _evt, win, cx| {
                                            this.is_creating = false;
                                            this.is_editing = false;

                                            cx.update_entity(&this.connection_form, |form, cx| {
                                                form.clear(win, cx);
                                                cx.notify();
                                            });

                                            if let Some(conn) = this.selected_connection.clone() {
                                                ConnectionState::connect(&conn, cx);
                                            }

                                            this.selected_connection = None;
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            })
            .when(show_form, |d| d.child(self.connection_form.clone()))
            .when(show_wecome, |d| {
                let version = div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .text_xs()
                    .opacity(0.6)
                    .child(format!("Version: {}", VERSION))
                    .child(Icon::new(IconName::Heart).xsmall());

                d.flex()
                    .items_center()
                    .justify_center()
                    .child(div().text_lg().child("PGUI"))
                    .child("Create or select a connection")
                    .child(version)
            });

        div().flex().size_full().child(sidebar).child(main)
    }
}
