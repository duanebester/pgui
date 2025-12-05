//! Files API client for uploading files to Anthropic.

use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct FileUploadResponse {
    id: String,
}

/// Get MIME type from file extension
fn get_mime_type(path: &PathBuf) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("md") => "text/plain",
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

/// Upload a file to the Anthropic Files API
pub fn upload_file(api_key: &str, path: &PathBuf) -> Result<String> {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");

    let mime_type = get_mime_type(path);

    let file_bytes = std::fs::read(path)
        .map_err(|e| anyhow!("Failed to read file {}: {}", path.display(), e))?;

    // Build multipart form data manually
    let boundary = "----AnthropicFileBoundary";
    let mut body = Vec::new();

    // Add file part
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
            file_name
        )
        .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", mime_type).as_bytes());
    body.extend_from_slice(&file_bytes);
    body.extend_from_slice(b"\r\n");

    // End boundary
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let response = smolhttp::Client::new("https://api.anthropic.com/v1/files")
        .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?
        .post()
        .headers(vec![
            ("x-api-key".to_string(), api_key.to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
            (
                "anthropic-beta".to_string(),
                "files-api-2025-04-14".to_string(),
            ),
            (
                "content-type".to_string(),
                format!("multipart/form-data; boundary={}", boundary),
            ),
        ])
        .body(body)
        .send()
        .map_err(|e| anyhow!("File upload request failed: {}", e))?;

    let response_text = response.text();

    if response_text.contains("\"error\"") {
        return Err(anyhow!("File upload error: {}", response_text));
    }

    let upload_response: FileUploadResponse =
        serde_json::from_str(&response_text).map_err(|e| {
            anyhow!(
                "Failed to parse upload response: {}. Response: {}",
                e,
                response_text
            )
        })?;

    tracing::debug!(
        "Uploaded file {} ({}) -> {}",
        path.display(),
        mime_type,
        upload_response.id
    );

    Ok(upload_response.id)
}
