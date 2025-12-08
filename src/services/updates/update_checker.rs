use anyhow::{Result, anyhow};
use serde::Deserialize;

const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/duanebester/pgui/releases/latest";

/// Represents the latest release info from GitHub
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub release_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubError {
    message: String,
}

/// Check if a newer version is available
pub async fn check_for_update() -> Result<Option<UpdateInfo>> {
    let current_version = env!("CARGO_PKG_VERSION");

    // Fetch latest release from GitHub
    let release = fetch_latest_release().await?;

    // Parse versions (strip 'v' prefix if present)
    let latest_tag = release.tag_name.trim_start_matches('v');
    let current = semver::Version::parse(current_version)
        .map_err(|e| anyhow!("Failed to parse current version: {}", e))?;
    let latest = semver::Version::parse(latest_tag)
        .map_err(|e| anyhow!("Failed to parse latest version: {}", e))?;

    if latest > current {
        Ok(Some(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: latest_tag.to_string(),
            release_url: release.html_url,
            release_notes: release.body,
        }))
    } else {
        Ok(None)
    }
}

async fn fetch_latest_release() -> Result<GitHubRelease> {
    smol::unblock(|| {
        let response = smolhttp::Client::new(GITHUB_RELEASES_URL)
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?
            .get()
            .headers(vec![
                ("User-Agent".to_string(), "pgui-update-checker".to_string()),
                (
                    "Accept".to_string(),
                    "application/vnd.github.v3+json".to_string(),
                ),
            ])
            .send()
            .map_err(|e| anyhow!("Failed to fetch release: {}", e))?;

        let body = response.text();

        // Check if it's an error response
        if body.contains("\"message\"") && body.contains("Not Found") {
            return Err(anyhow!("No releases found"));
        }

        // Try to parse as error first
        if let Ok(error) = serde_json::from_str::<GitHubError>(&body) {
            if error.message.contains("rate limit") {
                return Err(anyhow!("GitHub API rate limit exceeded"));
            }
            return Err(anyhow!("GitHub API error: {}", error.message));
        }

        serde_json::from_str(&body)
            .map_err(|e| anyhow!("Failed to parse GitHub response: {}. Body: {}", e, body))
    })
    .await
}
