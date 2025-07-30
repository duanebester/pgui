use anyhow::{Context, Result};
use async_std::{fs, path::Path};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::workspace::connections_panel::ConnectionInfo;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionStorage {
    pub connections: Vec<ConnectionInfo>,
    pub version: String,
}

impl Default for ConnectionStorage {
    fn default() -> Self {
        Self {
            connections: Vec::new(),
            version: "1.0".to_string(),
        }
    }
}

/// Load connections from the JSON file
pub async fn load_connections(file_path: &Path) -> Result<Vec<ConnectionInfo>> {
    if !file_path.exists() {
        // Return empty vec if file doesn't exist
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(file_path)
        .await
        .context("Failed to read connections file")?;

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    let storage: ConnectionStorage = serde_json::from_str(&content)
        .context("Failed to parse connections JSON")?;

    Ok(storage.connections)
}

/// Save connections to the JSON file
pub async fn save_connections(file_path: &Path, connections: &[ConnectionInfo]) -> Result<()> {
    // Ensure the parent directory exists
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .await
            .context("Failed to create pgui directory")?;
    }

    let storage = ConnectionStorage {
        connections: connections.to_vec(),
        version: "1.0".to_string(),
    };

    let json_content = serde_json::to_string_pretty(&storage)
        .context("Failed to serialize connections")?;

    fs::write(file_path, json_content)
        .await
        .context("Failed to write connections file")?;

    Ok(())
}

/// Get the default connections file path (~/.pgui/connections.json)
pub fn get_connections_file_path() -> Result<PathBuf> {
    let home_dir = std::env::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    
    Ok(home_dir.join(".pgui").join("connections.json"))
}

/// Add a new connection to the saved connections
pub async fn add_connection(connection: ConnectionInfo) -> Result<()> {
    let file_path = get_connections_file_path()?;
    let mut connections = load_connections(&file_path).await?;
    
    // Check if connection already exists (by hostname, username, database)
    let exists = connections.iter().any(|conn| {
        conn.hostname == connection.hostname 
            && conn.username == connection.username 
            && conn.database == connection.database
            && conn.port == connection.port
    });

    if !exists {
        connections.push(connection);
        save_connections(&file_path, &connections).await?;
    }

    Ok(())
}

/// Remove a connection from saved connections
pub async fn remove_connection(hostname: &str, username: &str, database: &str, port: &str) -> Result<bool> {
    let file_path = get_connections_file_path()?;
    let mut connections = load_connections(&file_path).await?;
    
    let initial_len = connections.len();
    connections.retain(|conn| {
        !(conn.hostname == hostname 
            && conn.username == username 
            && conn.database == database
            && conn.port == port)
    });

    let was_removed = connections.len() < initial_len;
    if was_removed {
        save_connections(&file_path, &connections).await?;
    }

    Ok(was_removed)
}

/// Update an existing connection
pub async fn update_connection(
    old_hostname: &str, 
    old_username: &str, 
    old_database: &str,
    old_port: &str,
    new_connection: ConnectionInfo
) -> Result<bool> {
    let file_path = get_connections_file_path()?;
    let mut connections = load_connections(&file_path).await?;
    
    let found = connections.iter_mut().find(|conn| {
        conn.hostname == old_hostname 
            && conn.username == old_username 
            && conn.database == old_database
            && conn.port == old_port
    });

    if let Some(conn) = found {
        *conn = new_connection;
        save_connections(&file_path, &connections).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Parse a PostgreSQL connection URL into ConnectionInfo
pub fn parse_connection_url(url: &str) -> Result<ConnectionInfo> {
    // Handle postgres:// or postgresql:// URLs
    let url = if url.starts_with("postgres://") {
        url.replacen("postgres://", "postgresql://", 1)
    } else {
        url.to_string()
    };

    let parsed = url::Url::parse(&url)
        .context("Invalid connection URL format")?;

    if parsed.scheme() != "postgresql" {
        return Err(anyhow::anyhow!("URL must use postgresql:// scheme"));
    }

    let hostname = parsed.host_str()
        .ok_or_else(|| anyhow::anyhow!("Missing hostname in URL"))?
        .to_string();

    let username = if parsed.username().is_empty() {
        "postgres".to_string()
    } else {
        parsed.username().to_string()
    };

    let password = parsed.password().unwrap_or("").to_string();

    let port = parsed.port().unwrap_or(5432).to_string();

    let database = if parsed.path().len() > 1 {
        parsed.path()[1..].to_string() // Remove leading '/'
    } else {
        username.clone() // Default to username if no database specified
    };

    Ok(ConnectionInfo {
        hostname,
        username,
        password,
        database,
        port,
    })
}

/// Convert ConnectionInfo to a PostgreSQL connection URL
pub fn connection_to_url(connection: &ConnectionInfo) -> String {
    if connection.password.is_empty() {
        format!(
            "postgresql://{}@{}:{}/{}",
            connection.username, connection.hostname, connection.port, connection.database
        )
    } else {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            connection.username, connection.password, connection.hostname, connection.port, connection.database
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::fs;
    use tempfile::tempdir;

    #[async_std::test]
    async fn test_save_and_load_connections() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_connections.json");

        let connections = vec![
            ConnectionInfo {
                hostname: "localhost".to_string(),
                username: "test_user".to_string(),
                password: "test_pass".to_string(),
                database: "test_db".to_string(),
                port: "5432".to_string(),
            },
            ConnectionInfo {
                hostname: "remote.example.com".to_string(),
                username: "admin".to_string(),
                password: "secret".to_string(),
                database: "production".to_string(),
                port: "5433".to_string(),
            },
        ];

        // Test saving
        save_connections(&file_path, &connections).await.unwrap();
        assert!(file_path.exists());

        // Test loading
        let loaded_connections = load_connections(&file_path).await.unwrap();
        assert_eq!(loaded_connections.len(), 2);
        assert_eq!(loaded_connections[0].hostname, "localhost");
        assert_eq!(loaded_connections[1].hostname, "remote.example.com");
    }

    #[async_std::test]
    async fn test_load_nonexistent_file() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("nonexistent.json");

        let connections = load_connections(&file_path).await.unwrap();
        assert!(connections.is_empty());
    }

    #[async_std::test]
    async fn test_parse_connection_url() {
        let url = "postgresql://myuser:mypass@localhost:5432/mydatabase";
        let conn = parse_connection_url(url).unwrap();

        assert_eq!(conn.hostname, "localhost");
        assert_eq!(conn.username, "myuser");
        assert_eq!(conn.password, "mypass");
        assert_eq!(conn.database, "mydatabase");
        assert_eq!(conn.port, "5432");
    }

    #[async_std::test]
    async fn test_parse_connection_url_minimal() {
        let url = "postgresql://localhost/testdb";
        let conn = parse_connection_url(url).unwrap();

        assert_eq!(conn.hostname, "localhost");
        assert_eq!(conn.username, "postgres");
        assert_eq!(conn.password, "");
        assert_eq!(conn.database, "testdb");
        assert_eq!(conn.port, "5432");
    }

    #[async_std::test]
    async fn test_connection_to_url() {
        let conn = ConnectionInfo {
            hostname: "localhost".to_string(),
            username: "testuser".to_string(),
            password: "testpass".to_string(),
            database: "testdb".to_string(),
            port: "5432".to_string(),
        };

        let url = connection_to_url(&conn);
        assert_eq!(url, "postgresql://testuser:testpass@localhost:5432/testdb");
    }

    #[async_std::test]
    async fn test_connection_to_url_no_password() {
        let conn = ConnectionInfo {
            hostname: "localhost".to_string(),
            username: "testuser".to_string(),
            password: "".to_string(),
            database: "testdb".to_string(),
            port: "5432".to_string(),
        };

        let url = connection_to_url(&conn);
        assert_eq!(url, "postgresql://testuser@localhost:5432/testdb");
    }
}