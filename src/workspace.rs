use crate::database_panel::DatabasePanel;
use crate::editor::EditorEvent;
use crate::{editor::Editor, results_panel::ResultsPanel};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt, Theme, ThemeMode,
    button::{Button, ButtonVariants as _},
    label::Label,
};

pub struct HeaderBar {}

impl HeaderBar {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {}
    }
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
    pub fn change_color_mode(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mode = match cx.theme().mode.is_dark() {
            true => ThemeMode::Light,
            false => ThemeMode::Dark,
        };
        Theme::change(mode, None, cx);
    }
}

impl Render for HeaderBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let logo = div()
            .flex()
            .flex_row()
            .gap_2()
            .items_center()
            .child(Icon::empty().path("icons/database-zap.svg"))
            .child(Label::new("PGUI").font_bold().text_sm());

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
            .on_click(cx.listener(Self::change_color_mode));

        div()
            .flex()
            .justify_between()
            .items_center()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .shadow_sm()
            .child(logo)
            .child(theme_toggle)
    }
}

pub struct Workspace {
    header_bar: Entity<HeaderBar>,
    database_panel: Entity<DatabasePanel>,
    editor: Entity<Editor>,
    results_panel: Entity<ResultsPanel>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let header_bar = HeaderBar::view(window, cx);
        let database_panel = DatabasePanel::view(window, cx);
        let editor = Editor::view(window, cx);
        let results_panel = ResultsPanel::view(window, cx);

        let _subscriptions =
            vec![
                cx.subscribe(&editor, |this, _, event: &EditorEvent, cx| match event {
                    EditorEvent::ExecuteQuery(query) => {
                        this.execute_query(query.clone(), cx);
                    }
                }),
            ];
        Self {
            header_bar,
            database_panel,
            editor,
            results_panel,
            _subscriptions,
        }
    }
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
    fn execute_query(&mut self, query: String, cx: &mut Context<Self>) {
        // Set editor to executing state
        self.editor.update(cx, |editor, cx| {
            editor.set_executing(true, cx);
        });

        // Get database manager from database panel
        let db_manager = self.database_panel.read(cx).db_manager.clone();

        cx.spawn(async move |this, cx| {
            let result = db_manager.execute_query(&query).await;

            this.update(cx, |this, cx| {
                // Update results panel
                this.results_panel.update(cx, |results, cx| {
                    results.update_result(result, cx);
                });

                // Set editor back to normal state
                this.editor.update(cx, |editor, cx| {
                    editor.set_executing(false, cx);
                });

                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.header_bar.clone())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .child(
                        div()
                            .w(px(300.0))
                            .h_full()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .child(self.database_panel.clone()),
                    )
                    .child(div().flex_1().h_full().child(self.editor.clone()))
                    .child(div().flex_1().child(self.results_panel.clone())),
            )
    }
}
