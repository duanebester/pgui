use gpui::*;

pub struct LLMState {
    pub llm_schema: Option<SharedString>,
}

impl Global for LLMState {}

impl LLMState {
    pub fn init(cx: &mut App) {
        let this = LLMState { llm_schema: None };
        cx.set_global(this);
    }
}
