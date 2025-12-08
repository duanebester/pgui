use crate::services::QueryResult;
use anyhow::Result;
use futures::StreamExt;
use serde_json::{Map, Value};
use sqlx::postgres::PgRow;
use sqlx::{Column, Row, ValueRef};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Stream rows to NDJSON (newline-delimited JSON) format
/// Each line is a valid JSON object - perfect for huge datasets
#[allow(dead_code)]
pub async fn stream_to_ndjson<S>(mut row_stream: S, output_path: &Path) -> Result<u64>
where
    S: futures::Stream<Item = Result<PgRow, sqlx::Error>> + Unpin,
{
    let file = std::fs::File::create(output_path)?;
    let mut writer = BufWriter::with_capacity(64 * 1024, file);

    let mut row_count = 0u64;

    while let Some(row_result) = row_stream.next().await {
        let row = row_result?;

        let mut obj = Map::new();
        for (i, col) in row.columns().iter().enumerate() {
            let value = extract_json_value(&row, i, col);
            obj.insert(col.name().to_string(), value);
        }

        // Write one JSON object per line
        serde_json::to_writer(&mut writer, &Value::Object(obj))?;
        writer.write_all(b"\n")?;

        row_count += 1;

        // Flush periodically
        if row_count % 10_000 == 0 {
            writer.flush()?;
        }
    }

    writer.flush()?;
    Ok(row_count)
}

fn extract_json_value(row: &PgRow, index: usize, _col: &sqlx::postgres::PgColumn) -> Value {
    if let Ok(raw) = row.try_get_raw(index) {
        if raw.is_null() {
            return Value::Null;
        }
    }

    // Try types in order of likelihood
    if let Ok(v) = row.try_get::<i64, _>(index) {
        return Value::from(v);
    }
    if let Ok(v) = row.try_get::<f64, _>(index) {
        return Value::from(v);
    }
    if let Ok(v) = row.try_get::<bool, _>(index) {
        return Value::from(v);
    }
    if let Ok(v) = row.try_get::<String, _>(index) {
        return Value::String(v);
    }

    Value::Null
}

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
