use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, ThemeMode, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
};

use crate::{
    services::{check_for_update, updates::UpdateInfo},
    themes::*,
};

pub struct HeaderBar {
    update_available: Option<UpdateInfo>,
}

impl HeaderBar {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let this = Self {
            update_available: None,
        };

        // Check for updates on startup
        cx.spawn(async move |this, cx| match check_for_update().await {
            Ok(Some(update_info)) => {
                tracing::info!(
                    "Update available: {} -> {}",
                    update_info.current_version,
                    update_info.latest_version
                );
                let _ = this.update(cx, |this, cx| {
                    this.update_available = Some(update_info);
                    cx.notify();
                });
            }
            Ok(None) => {
                tracing::debug!("No update available");
            }
            Err(e) => {
                tracing::warn!("Failed to check for updates: {}", e);
            }
        })
        .detach();

        this
    }
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
    pub fn change_mode(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        tracing::debug!("Current mode: {:?}", cx.theme().mode);
        let new_mode = if cx.theme().mode.is_dark() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        change_color_mode(new_mode, window, cx);
    }

    fn open_release_page(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(update_info) = &self.update_available {
            cx.open_url(&update_info.release_url);
        }
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

        // Update button - only show if update is available
        let update_button = self.update_available.as_ref().map(|info| {
            let label: SharedString = format!("v{} available!", info.latest_version).into();
            Button::new("update-available")
                .icon(Icon::empty().path("icons/cloud-download.svg"))
                .small()
                .tooltip(label)
                .ghost()
                .on_click(cx.listener(Self::open_release_page))
        });

        TitleBar::new().child(
            h_flex()
                .w_full()
                .pr_2()
                .justify_between()
                .child(Label::new("PGUI").text_xs())
                .child(
                    div()
                        .pr(px(5.0))
                        .flex()
                        .items_center()
                        .when(self.update_available.is_some(), |d| {
                            d.child(update_button.unwrap())
                        })
                        .child(theme_toggle)
                        .child(github_button),
                ),
        )
    }
}
