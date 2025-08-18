use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme as _, IconName, Sizable as _, ThemeMode,
    button::{Button, ButtonVariants as _},
    label::Label,
};

use crate::themes::*;

const TITLE_BAR_LEFT_PADDING: Pixels = px(80.);

pub struct HeaderBar {}

impl HeaderBar {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {}
    }
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
    pub fn change_mode(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        println!("Current mode: {:?}", cx.theme().mode);
        let new_mode = if cx.theme().mode.is_dark() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        change_color_mode(new_mode, window, cx);
    }
}

impl Render for HeaderBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme_toggle = Button::new("theme-mode")
            .map(|this| {
                if cx.theme().mode.is_dark() {
                    this.icon(IconName::Sun)
                } else {
                    this.icon(IconName::Moon)
                }
            })
            .small()
            .ghost()
            .on_click(cx.listener(Self::change_mode));

        let github_button = Button::new("github")
            .icon(IconName::GitHub)
            .small()
            .ghost()
            .on_click(|_, _, cx| cx.open_url("https://github.com/duanebester/pgui"));

        div()
            .id("header-bar")
            .border_b_1()
            .bg(cx.theme().title_bar)
            .border_color(cx.theme().border)
            .pl(TITLE_BAR_LEFT_PADDING)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .p_1()
                    .child(Label::new("PGUI").text_xs())
                    .child(
                        div()
                            .pr(px(5.0))
                            .flex()
                            .items_center()
                            .child(theme_toggle)
                            .child(github_button),
                    ),
            )
    }
}
