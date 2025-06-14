use gpui::*;
use gpui_component::button::{Button, ButtonGroup, ButtonVariants};
use gpui_component::{ActiveTheme, Icon, Selectable, Sizable};

pub struct FooterBar {
    connections_active: bool,
    tables_active: bool,
}

pub enum FooterBarEvent {
    ShowConnections,
    ShowTables,
}

impl EventEmitter<FooterBarEvent> for FooterBar {}

impl FooterBar {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            connections_active: true,
            tables_active: false,
        }
    }
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Render for FooterBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connections_button = Button::new("connections_panel")
            .icon(Icon::empty().path("icons/cable.svg"))
            .small()
            .selected(self.connections_active.clone())
            .ghost()
            .tooltip("Show Connections");

        let tables_button = Button::new("tables_panel")
            .icon(Icon::empty().path("icons/table-properties.svg"))
            .small()
            .selected(self.tables_active.clone())
            .ghost()
            .tooltip("Show Database Tables");

        let controls = ButtonGroup::new("controls-toggle-group")
            .ghost()
            .compact()
            .child(connections_button)
            .child(tables_button)
            .on_click(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                this.connections_active = selected.contains(&0);
                this.tables_active = selected.contains(&1);
                if selected.contains(&0) {
                    cx.emit(FooterBarEvent::ShowConnections);
                } else if selected.contains(&1) {
                    cx.emit(FooterBarEvent::ShowTables);
                }
                cx.notify();
            }));

        let footer = div()
            .border_t_1()
            .text_xs()
            .bg(cx.theme().title_bar)
            .border_color(cx.theme().border)
            .flex()
            .flex_row()
            .justify_start()
            .items_center()
            .p_2()
            .child(controls);

        footer
    }
}
