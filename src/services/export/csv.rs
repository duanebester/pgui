use crate::services::QueryResult;
use anyhow::Result;
use csv::Writer;
use futures::StreamExt;
use sqlx::postgres::PgRow;
use sqlx::{Column, Row, ValueRef};
use std::path::Path;

/// Stream rows directly to a CSV file without holding everything in memory
#[allow(dead_code)]
pub async fn stream_to_csv<S>(mut row_stream: S, output_path: &Path) -> Result<u64>
where
    S: futures::Stream<Item = Result<PgRow, sqlx::Error>> + Unpin,
{
    let file = std::fs::File::create(output_path)?;
    let buf_writer = std::io::BufWriter::with_capacity(64 * 1024, file); // 64KB buffer
    let mut wtr = Writer::from_writer(buf_writer);

    let mut row_count = 0u64;
    let mut headers_written = false;

    while let Some(row_result) = row_stream.next().await {
        let row = row_result?;

        // Write headers on first row
        if !headers_written {
            let headers: Vec<&str> = row.columns().iter().map(|c| c.name()).collect();
            wtr.write_record(&headers)?;
            headers_written = true;
        }

        // Write row values
        let values: Vec<String> = row
            .columns()
            .iter()
            .enumerate()
            .map(|(i, col)| extract_value(&row, i, col))
            .collect();
        wtr.write_record(&values)?;

        row_count += 1;

        // Flush periodically to avoid memory buildup
        if row_count % 10_000 == 0 {
            wtr.flush()?;
        }
    }

    wtr.flush()?;
    Ok(row_count)
}

fn extract_value(row: &PgRow, index: usize, _col: &sqlx::postgres::PgColumn) -> String {
    // Check for NULL first
    if let Ok(raw) = row.try_get_raw(index) {
        if raw.is_null() {
            return String::new(); // CSV NULL representation
        }
    }

    // Try string first, then specific types
    row.try_get::<String, _>(index)
        .or_else(|_| row.try_get::<i64, _>(index).map(|v| v.to_string()))
        .or_else(|_| row.try_get::<f64, _>(index).map(|v| v.to_string()))
        .or_else(|_| row.try_get::<bool, _>(index).map(|v| v.to_string()))
        .unwrap_or_default()
}

pub fn export_to_csv(result: &QueryResult) -> Result<String> {
    let mut wtr = Writer::from_writer(vec![]);

    // Header row
    let headers: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
    wtr.write_record(&headers)?;

    // Data rows
    for row in &result.rows {
        let values: Vec<&str> = row.cells.iter().map(|c| c.value.as_str()).collect();
        wtr.write_record(&values)?;
    }

    let bytes = wtr.into_inner()?;
    Ok(String::from_utf8(bytes)?)
}
