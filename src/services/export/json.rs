use crate::services::QueryResult;
use anyhow::Result;
use serde_json::{Map, Value};

pub fn export_to_json(result: &QueryResult) -> Result<String> {
    let rows: Vec<Value> = result
        .rows
        .iter()
        .map(|row| {
            let mut obj = Map::new();
            for cell in &row.cells {
                let value = if cell.is_null {
                    Value::Null
                } else {
                    // Try to parse as number, otherwise keep as string
                    cell.value
                        .parse::<i64>()
                        .map(Value::from)
                        .or_else(|_| cell.value.parse::<f64>().map(Value::from))
                        .unwrap_or_else(|_| Value::String(cell.value.clone()))
                };
                obj.insert(cell.column_metadata.name.clone(), value);
            }
            Value::Object(obj)
        })
        .collect();

    Ok(serde_json::to_string_pretty(&rows)?)
}
