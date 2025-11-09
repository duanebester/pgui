use async_std::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub name: String,
    pub hostname: String,
    pub username: String,
    pub password: String,
    pub database: String,
    pub port: usize,
}

impl ConnectionInfo {
    pub fn to_connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username, self.password, self.hostname, self.port, self.database
        )
    }
}

impl Default for ConnectionInfo {
    fn default() -> Self {
        Self {
            name: "Test".to_string(),
            hostname: "localhost".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
            database: "test".to_string(),
            port: 5432,
        }
    }
}

pub async fn load_connections() -> Vec<ConnectionInfo> {
    let default = vec![];
    if let Some(path) = std::env::home_dir() {
        let project_dir = path.join(".pgui");
        let connections_file = project_dir.join("connections.json");
        if !connections_file.exists() {
            return default;
        }
        let content = match fs::read_to_string(connections_file).await {
            Ok(content) => content,
            Err(_) => return default,
        };
        if content.trim().is_empty() {
            return default;
        }
        serde_json::from_str(&content).unwrap_or(default)
    } else {
        default
    }
}
