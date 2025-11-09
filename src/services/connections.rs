use async_std::fs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub username: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
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
    pub fn new(
        name: String,
        hostname: String,
        username: String,
        password: String,
        database: String,
        port: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            hostname,
            username,
            password,
            database,
            port,
        }
    }
}

impl Default for ConnectionInfo {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
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
        // Deserialize and ensure all connections have UUIDs
        let mut connections: Vec<ConnectionInfo> =
            serde_json::from_str(&content).unwrap_or(default);
        for conn in connections.iter_mut() {
            if conn.id.is_nil() {
                conn.id = Uuid::new_v4();
            }
        }
        connections
    } else {
        default
    }
}
