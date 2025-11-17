use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme, Icon, Selectable as _, Sizable as _};

use crate::services::ConnectionInfo;
use crate::state::{ConnectionState, ConnectionStatus};

pub struct FooterBar {
    active_connection: Option<ConnectionInfo>,
    tables_active: bool,
    is_connected: bool,
    _subscriptions: Vec<Subscription>,
}

pub enum FooterBarEvent {
    HideTables,
    ShowTables,
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
                    cx.emit(FooterBarEvent::ShowTables);
                } else {
                    cx.emit(FooterBarEvent::HideTables);
                }
                cx.notify();
            }));

        let controls = div()
            .flex()
            .flex_row()
            .justify_center()
            .items_center()
            .gap_2()
            .when(!self.is_connected.clone(), |d| d.invisible())
            .child(tables_button);

        let footer = div()
            .border_t_1()
            .text_xs()
            .bg(cx.theme().title_bar)
            .border_color(cx.theme().border)
            .flex()
            .flex_row()
            .justify_start()
            .items_center()
            .py_1()
            .px_2()
            .child(controls);

        footer
    }
}
