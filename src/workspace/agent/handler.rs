use async_channel::{Receiver, Sender};
use gpui::{AppContext, AsyncApp, WeakEntity};

use crate::{
    services::agent::{
        Agent, AgentRequest, AgentResponse, ContentBlock, UiMessage, create_get_schema_tool,
        create_get_table_columns_tool, create_get_tables_tool,
    },
    workspace::agent::{panel::AgentPanel, tools::execute_tools},
};

pub async fn handle_outgoing(
    outgoing_rx: Receiver<AgentRequest>,
    incoming_tx: Sender<AgentResponse>,
) {
    if let Some(mut agent) = Agent::builder()
        .system_prompt(
            "You are a helpful, succint, postgres assistant with access to database tools. \
          Please respond only in markdown and no emojis. \
          "
            .to_string(),
        )
        .max_tokens(4096)
        .build(vec![
            create_get_schema_tool(),
            create_get_tables_tool(),
            create_get_table_columns_tool(),
        ])
        .ok()
    {
        while let Ok(request) = outgoing_rx.recv().await {
            match request {
                AgentRequest::Chat(content) => {
                    // Start a new user message
                    let user_content = vec![ContentBlock::Text { text: content }];

                    match agent.chat_step(user_content).await {
                        Ok(response) => {
                            let _ = incoming_tx.try_send(response);
                        }
                        Err(e) => {
                            let _ = incoming_tx.try_send(AgentResponse::Error(format!("{}", e)));
                        }
                    }
                }
                AgentRequest::ToolResults(results) => {
                    // Submit tool results and continue the conversation
                    agent.submit_tool_results(results);

                    // Continue with empty user content (tool results are already added)
                    match agent.chat_step(vec![]).await {
                        Ok(response) => {
                            let _ = incoming_tx.try_send(response);
                        }
                        Err(e) => {
                            let _ = incoming_tx.try_send(AgentResponse::Error(format!("{}", e)));
                        }
                    }
                }
                AgentRequest::ClearHistory => {
                    agent.clear_conversation();
                }
                AgentRequest::SetModel(model) => {
                    // Update the agent's model
                    println!("Setting agent model to: {}", model);
                    agent.set_model(model);
                    // Clear conversation when model changes
                    agent.clear_conversation();
                }
            }
        }
    } else {
        println!("Failed to build agent");
        let _ = incoming_tx.try_send(AgentResponse::Error(
            "Failed to initialize agent".to_string(),
        ));
    }
}

pub async fn handle_incoming(
    this: WeakEntity<AgentPanel>,
    incoming_rx: Receiver<AgentResponse>,
    outgoing_tx: Sender<AgentRequest>,
    cx: &mut AsyncApp,
) {
    loop {
        let incoming_response = incoming_rx.recv().await;
        match incoming_response {
            Ok(response) => {
                // Check if this response means we're done processing
                let is_done = response.is_done();

                match response {
                    AgentResponse::TextResponse { text, .. } => {
                        if let Some(view) = this.upgrade() {
                            let _ = cx.update_entity(&view, |this, cx| {
                                this.add_message(UiMessage::assistant(text), cx);
                                // Clear loading state only if done
                                if is_done {
                                    this.set_loading(false, cx);
                                }
                            });
                        }
                    }
                    AgentResponse::ToolCallRequest {
                        text, tool_calls, ..
                    } => {
                        // Execute tools with database access
                        let results = execute_tools(tool_calls.clone(), &cx).await;

                        if let Some(view) = this.upgrade() {
                            let _ = cx.update_entity(&view, |this, cx| {
                                // Display the agent's explanation text if present
                                if let Some(text) = text {
                                    this.add_message(UiMessage::assistant(text), cx);
                                }

                                // Display tool calls
                                for tool_call in &tool_calls {
                                    this.add_message(
                                        UiMessage::tool_call(
                                            tool_call.name.clone(),
                                            tool_call.input.clone(),
                                        ),
                                        cx,
                                    );
                                }

                                // Clear loading state only if done (unlikely for tool calls)
                                if is_done {
                                    this.set_loading(false, cx);
                                }
                            });
                        }

                        // Send results back to agent
                        let _ = outgoing_tx.try_send(AgentRequest::ToolResults(results));
                    }
                    AgentResponse::Error(err) => {
                        if let Some(view) = this.upgrade() {
                            let _ = cx.update_entity(&view, |this, cx| {
                                this.add_message(UiMessage::error(err), cx);
                                // Always clear loading state on error
                                this.set_loading(false, cx);
                            });
                        }
                    }
                }
            }
            Err(e) => {
                println!("Channel error: {}", e);
                if let Some(view) = this.upgrade() {
                    let _ = cx.update_entity(&view, |this, cx| {
                        // TODO: notify of error
                        this.set_loading(false, cx);
                    });
                }
                break;
            }
        }
    }
}
