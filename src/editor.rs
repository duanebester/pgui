use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    highlighter::Language,
    input::{InputEvent, InputState, TabSize, TextInput},
    v_flex,
};

pub enum EditorEvent {
    ExecuteQuery(String),
}

impl EventEmitter<EditorEvent> for Editor {}

pub struct Editor {
    input_state: Entity<InputState>,
    is_executing: bool,
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
                .placeholder("Enter your SQL query here...")
        });

        let _subscribes = vec![cx.subscribe(&input_state, |_, _, _: &InputEvent, cx| {
            cx.notify();
        })];

        Self {
            input_state,
            is_executing: false,
            _subscribes,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn execute_query(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let query = self.input_state.read(cx).value().clone();
        if !query.trim().is_empty() {
            cx.emit(EditorEvent::ExecuteQuery(query.to_string()));
        }
    }

    pub fn set_executing(&mut self, executing: bool, cx: &mut Context<Self>) {
        self.is_executing = executing;
        cx.notify();
    }
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let execute_button = Button::new("execute-query")
            .label(if self.is_executing {
                "Executing..."
            } else {
                "Execute"
            })
            .icon(Icon::empty().path("icons/play.svg"))
            .small()
            .primary()
            .disabled(self.is_executing)
            .on_click(cx.listener(Self::execute_query));

        let toolbar = h_flex()
            .justify_between()
            .items_center()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                h_flex().gap_2().items_center().child(execute_button),
            );

        v_flex().size_full().child(toolbar).child(
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
