use gpui::*;
use gpui_component::{
    highlighter::Language,
    input::{InputEvent, InputState, TabSize, TextInput},
    v_flex,
};

pub struct Editor {
    input_state: Entity<InputState>,
    _subscribes: Vec<Subscription>,
}

impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let default_language = Language::Sql;
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(default_language.name())
                .line_number(true)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .placeholder("Enter your code here...")
        });

        let _subscribes = vec![cx.subscribe(&input_state, |_, _, _: &InputEvent, cx| {
            cx.notify();
        })];

        Self {
            input_state,
            _subscribes,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().size_full().child(
            div()
                .id("editor-content")
                .w_full()
                .flex_1()
                .p_4()
                .font_family("Monaco")
                .text_size(px(12.))
                .child(TextInput::new(&self.input_state).h_full()),
        )
    }
}
