use gpui::AsyncApp;

use crate::{
    services::agent::{ToolCallData, ToolResultData},
    state::ConnectionState,
};

/// Execute tools with access to context
/// This is where you'll add database access, file system, etc.
pub async fn execute_tools(tool_calls: Vec<ToolCallData>, cx: &AsyncApp) -> Vec<ToolResultData> {
    let mut results = Vec::new();
    for call in tool_calls {
        // Example: Execute tools based on name
        let result = match call.name.as_str() {
            "get_schema" => {
                let table_name = call
                    .input
                    .get("table_name")
                    .and_then(|v| v.as_str())
                    .map(|v| vec![v.to_string()]);

                // Helper to create error result (avoids duplication)
                let error_result = || ToolResultData {
                    tool_use_id: call.id.clone(),
                    content: "Failed to fetch schema".to_string(),
                    is_error: true,
                };

                match cx.read_global::<ConnectionState, _>(|state, _cx| state.db_manager.clone()) {
                    Ok(db) => match db.get_schema(table_name).await {
                        Ok(schema) => {
                            let formatted = db.format_schema_for_llm(&schema);
                            ToolResultData {
                                tool_use_id: call.id,
                                content: formatted,
                                is_error: false,
                            }
                        }
                        Err(_) => error_result(),
                    },
                    Err(_) => error_result(),
                }
            }

            _ => ToolResultData {
                tool_use_id: call.id,
                content: format!("Unknown tool: {}", call.name),
                is_error: true,
            },
        };

        results.push(result);
    }

    results
}
