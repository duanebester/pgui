use async_channel::{Sender, unbounded};
use gpui::{
    AnyElement, App, AppContext, ClickEvent, Context, Div, Entity, IntoElement, ListAlignment,
    ListState, ParentElement, Render, SharedString, Styled as _, Window, div, list, px,
};
use gpui_component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    text::TextView,
};

use crate::{
    services::agent::{AgentRequest, AgentResponse, MessageRole, UiMessage},
    workspace::agent::handler::{handle_incoming, handle_outgoing},
};

pub struct MessageState {
    messages: Vec<UiMessage>,
}

pub struct AgentPanel {
    textarea: Entity<InputState>,
    message_state: Entity<MessageState>,
    outgoing_tx: Sender<AgentRequest>,
    list_state: ListState,
    is_loading: bool,
}

impl AgentPanel {
    fn render_tool_call(&mut self, item: UiMessage) -> Div {
        div()
            .p_2()
            .flex()
            .w_full()
            .justify_start()
            .overflow_hidden()
            .items_center()
            .gap_2()
            .child(Icon::empty().path("icons/hammer.svg"))
            .child(item.content)
    }

    fn render_assistant(
        &mut self,
        ix: usize,
        item: UiMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let id: SharedString = format!("chat-{}", ix).into();
        div()
            .p_2()
            .child(TextView::markdown(id, item.content, window, cx).selectable(true))
    }

    fn render_user(
        &mut self,
        ix: usize,
        item: UiMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let id: SharedString = format!("chat-{}", ix).into();
        div()
            .p_2()
            .border_1()
            .bg(cx.theme().list_even)
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(TextView::markdown(id, item.content, window, cx).selectable(true))
    }

    fn render_entry(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = self.message_state.read(cx).messages.clone();
        if items.len() == 0 {
            return div().into_any_element();
        }
        let item = items.get(ix).unwrap().clone();
        let elem = match item.role {
            MessageRole::ToolCall => self.render_tool_call(item),
            MessageRole::ToolResult => div(),
            MessageRole::Assistant => self.render_assistant(ix, item, window, cx),
            MessageRole::System => self.render_assistant(ix, item, window, cx),
            MessageRole::User => self.render_user(ix, item, window, cx),
        };

        div().p_1().child(elem).into_any_element()
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let textarea = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(1, 5)
                .soft_wrap(true)
                .placeholder("Ask me anything about your database...")
        });

        let (incoming_tx, incoming_rx) = unbounded::<AgentResponse>();
        let (outgoing_tx, outgoing_rx) = unbounded::<AgentRequest>();

        // Initialize state with empty messages
        let message_state = cx.new(|_cx| MessageState { messages: vec![] });

        // Spawn the agent message handler
        cx.background_executor()
            .spawn(handle_outgoing(outgoing_rx, incoming_tx))
            .detach();

        // Spawn task to handle incoming responses from agent
        let outgoing_tx_clone = outgoing_tx.clone();
        cx.spawn(async move |this, cx| {
            handle_incoming(this, incoming_rx, outgoing_tx_clone, cx).await;
        })
        .detach();

        let list_state = ListState::new(4, ListAlignment::Top, px(20.));

        cx.observe(&message_state, |this: &mut AgentPanel, _event, cx| {
            let items = this.message_state.read(cx).messages.clone();
            this.list_state = ListState::new(items.len(), ListAlignment::Top, px(20.));
            cx.notify();
        })
        .detach();

        Self {
            textarea,
            message_state,
            outgoing_tx,
            list_state,
            is_loading: false,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn add_message(&mut self, message: UiMessage, cx: &mut Context<Self>) {
        cx.update_entity(&self.message_state, |state, cx| {
            state.messages.push(message);
            cx.notify();
        });
    }

    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.is_loading = loading;
        cx.notify();
    }

    fn on_send_message(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.textarea.read(cx).text().to_string();
        if text.trim().is_empty() {
            return;
        }

        // Send chat request to agent
        let result = self.outgoing_tx.try_send(AgentRequest::Chat(text.clone()));
        match result {
            Ok(_) => {
                println!("Message sent successfully");
                // Add user message to display
                self.add_message(UiMessage::user(text), cx);
                self.set_loading(true, cx);
            }
            Err(e) => {
                println!("Failed to send message: {}", e);
                self.add_message(UiMessage::error(format!("Failed to send: {}", e)), cx);
            }
        }

        // Clear the textarea
        self.textarea.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });

        cx.notify();
    }
}

impl Render for AgentPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div().p_2().size_full().flex().child(
                    list(
                        self.list_state.clone(),
                        cx.processor(|this, ix, window, cx| this.render_entry(ix, window, cx)),
                    )
                    .size_full(),
                ),
            )
            .child(
                div()
                    .p_2()
                    .border_t_1()
                    .border_color(cx.theme().input)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .w_full()
                            .pt_2()
                            .child(Input::new(&self.textarea).h(px(120.0)).appearance(false)),
                    )
                    .child(
                        div().flex().justify_end().child(
                            Button::new("btn-send")
                                .tooltip("send")
                                .ghost()
                                .p_2()
                                .small()
                                .loading(self.is_loading)
                                .icon(Icon::empty().path("icons/send.svg"))
                                .on_click(cx.listener(Self::on_send_message)),
                        ),
                    ),
            )
    }
}
