use crate::services::QueryResult;
use anyhow::Result;
use csv::Writer;

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
