use crate::database_panel::DatabasePanel;
use crate::editor::EditorEvent;
use crate::header_bar::HeaderBar;
use crate::{editor::Editor, results_panel::ResultsPanel};

use gpui::*;
use gpui_component::{
    ActiveTheme as _,
    resizable::{ResizableState, resizable_panel, v_resizable},
};

pub struct Workspace {
    resize_state: Entity<ResizableState>,
    header_bar: Entity<HeaderBar>,
    database_panel: Entity<DatabasePanel>,
    editor: Entity<Editor>,
    results_panel: Entity<ResultsPanel>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let header_bar = HeaderBar::view(window, cx);
        let resize_state = ResizableState::new(cx);
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
            resize_state,
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
                    .child(
                        v_resizable("resizable", self.resize_state.clone())
                            .child(
                                resizable_panel()
                                    .size(px(500.))
                                    .size_range(px(200.)..px(800.))
                                    .child(self.editor.clone()),
                            )
                            .child(resizable_panel().child(self.results_panel.clone())),
                    ),
            )
    }
}
