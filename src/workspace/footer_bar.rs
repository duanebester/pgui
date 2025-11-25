use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::label::Label;
use gpui_component::{ActiveTheme, Icon, IconName, Selectable as _, Sizable as _};

use crate::services::ConnectionInfo;
use crate::state::{ConnectionState, ConnectionStatus};

pub struct FooterBar {
    active_connection: Option<ConnectionInfo>,
    tables_active: bool,
    agent_active: bool,
    is_connected: bool,
    _subscriptions: Vec<Subscription>,
}

pub enum FooterBarEvent {
    ToggleTables(bool), // true = show
    ToggleAgent(bool),
}

impl EventEmitter<FooterBarEvent> for FooterBar {}

impl FooterBar {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _subscriptions = vec![cx.observe_global::<ConnectionState>(move |this, cx| {
            let state = cx.global::<ConnectionState>();
            this.is_connected = state.connection_state.clone() == ConnectionStatus::Connected;
            this.active_connection = state.active_connection.clone();
            cx.notify();
        })];

        Self {
            active_connection: None,
            tables_active: true,
            agent_active: false,
            is_connected: false,
            _subscriptions,
        }
    }
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Render for FooterBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tables_button = Button::new("tables_button")
            .icon(Icon::empty().path("icons/panel-left.svg"))
            .small()
            .ghost()
            .selected(self.tables_active.clone())
            .tooltip("Toggle Tables Panel")
            .on_click(cx.listener(|this, _evt, _win, cx| {
                this.tables_active = !this.tables_active;
                if this.tables_active {
                    cx.emit(FooterBarEvent::ToggleTables(true));
                } else {
                    cx.emit(FooterBarEvent::ToggleTables(false));
                }
                cx.notify();
            }));

        let agent_button = Button::new("agent_button")
            .icon(IconName::Bot)
            .small()
            .ghost()
            .selected(self.agent_active.clone())
            .tooltip("Toggle Agent Panel")
            .on_click(cx.listener(|this, _evt, _win, cx| {
                this.agent_active = !this.agent_active;
                if this.agent_active {
                    cx.emit(FooterBarEvent::ToggleAgent(true));
                } else {
                    cx.emit(FooterBarEvent::ToggleAgent(false));
                }
                cx.notify();
            }));

        let connection_url = self
            .active_connection
            .clone()
            .map(|x| format!("{}@{}:{}", x.username, x.hostname, x.port));

        let _connection_status = div()
            .flex()
            .items_center()
            .justify_center()
            .gap_1()
            .pr_1()
            .when(connection_url.clone().is_some(), |d| {
                d.child(
                    Label::new(connection_url.clone().unwrap())
                        .italic()
                        .text_xs(),
                )
                .child(Icon::empty().path("icons/power.svg"))
                .text_color(cx.theme().primary)
            })
            .when(connection_url.is_none(), |d| {
                d.child(Label::new("Disconnected").italic().text_xs())
                    .child(Icon::empty().path("icons/power.svg"))
                    .text_color(cx.theme().foreground)
                    .opacity(0.6)
            });

        let controls = div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .gap_2()
            .when(!self.is_connected.clone(), |d| d.invisible())
            .child(tables_button)
            .child(agent_button);

        let footer = div()
            .border_t_1()
            .text_xs()
            .bg(cx.theme().title_bar)
            .border_color(cx.theme().border)
            .w_full()
            .py_1()
            .px_2()
            .child(controls);

        footer
    }
}
