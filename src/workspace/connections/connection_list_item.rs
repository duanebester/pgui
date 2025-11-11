use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, IndexPath, Selectable, StyledExt, h_flex, label::Label, list::ListItem,
    v_flex,
};

use crate::services::ConnectionInfo;

#[derive(IntoElement)]
pub struct ConnectionListItem {
    base: ListItem,
    ix: IndexPath,
    connection: ConnectionInfo,
    selected: bool,
}

impl ConnectionListItem {
    pub fn new(
        id: impl Into<ElementId>,
        connection: ConnectionInfo,
        ix: IndexPath,
        selected: bool,
    ) -> Self {
        Self {
            connection,
            ix,
            base: ListItem::new(id),
            selected,
        }
    }
}

impl Selectable for ConnectionListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for ConnectionListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color = if self.selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let bg_color = if self.selected {
            cx.theme().list_active
        } else if self.ix.row % 2 == 0 {
            cx.theme().list
        } else {
            cx.theme().list_even
        };

        self.base
            .px_3()
            .py_2()
            .overflow_x_hidden()
            .bg(bg_color)
            .when(self.selected, |this| {
                this.border_color(cx.theme().list_active_border)
            })
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .text_color(text_color)
                    .child(
                        v_flex()
                            .gap_1()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(
                                Label::new(self.connection.name.clone())
                                    .font_semibold()
                                    .whitespace_nowrap(),
                            )
                            .child(
                                Label::new(format!(
                                    "{}@{}:{}/{}",
                                    self.connection.username,
                                    self.connection.hostname,
                                    self.connection.port,
                                    self.connection.database
                                ))
                                .text_xs()
                                .text_color(text_color.opacity(0.6))
                                .whitespace_nowrap(),
                            ),
                    ),
            )
    }
}
