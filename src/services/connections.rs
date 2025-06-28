use std::fs;

use anyhow::Error;
use gpui::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Connection {
    username: String,
    password: String,
    hostname: String,
    database: String,
    port: usize,
}

// Add conversion methods
impl From<&SavedConnection> for Connection {
    fn from(saved: &SavedConnection) -> Self {
        Self {
            username: saved.username.to_string(),
            password: saved.password.to_string(),
            hostname: saved.hostname.to_string(),
            database: saved.database.to_string(),
            port: saved.port,
        }
    }
}

impl Connection {
    pub fn try_parse(db_url: String) -> Result<Self, Error> {
        use url::Url;

        let url = Url::parse(&db_url)?;

        // Check if scheme is postgres or postgresql
        if url.scheme() != "postgres" && url.scheme() != "postgresql" {
            return Err(anyhow::anyhow!("Invalid scheme: expected 'postgres' or 'postgresql', got '{}'", url.scheme()));
        }

        // Extract hostname
        let hostname = url.host_str()
            .ok_or_else(|| anyhow::anyhow!("Missing hostname in database URL"))?
            .to_string();

        // Extract port, default to 5432 if not specified
        let port = url.port().unwrap_or(5432) as usize;

        // Extract database name from path, removing leading slash
        let database = url.path()
            .strip_prefix('/')
            .unwrap_or("")
            .to_string();

        if database.is_empty() {
            return Err(anyhow::anyhow!("Missing database name in URL"));
        }

        // Extract username and password
        let username = url.username().to_string();
        let password = url.password().unwrap_or("").to_string();

        if username.is_empty() {
            return Err(anyhow::anyhow!("Missing username in database URL"));
        }

        Ok(Self {
            username,
            password,
            hostname,
            database,
            port,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedConnection {
    id: usize,
    name: SharedString,
    username: SharedString,
    password: SharedString,
    hostname: SharedString,
    database: SharedString,
    port: usize,
}

impl SavedConnection {
    pub fn from_connection(conn: Connection, id: usize, name: impl Into<SharedString>) -> Self {
        Self {
            id,
            name: name.into(),
            username: conn.username.into(),
            password: conn.password.into(),
            hostname: conn.hostname.into(),
            database: conn.database.into(),
            port: conn.port,
        }
    }

    pub fn to_connection(&self) -> Connection {
        Connection::from(self)
    }
}

pub struct SavedConnections {
    pub connections: Vec<SavedConnection>,
}

pub fn load_connections() -> Vec<SavedConnection> {
    // Get home directory and construct path to connections file
    let home_dir = match dirs::home_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("Could not determine home directory");
            return vec![];
        }
    };

    // Check to see if we have <home_dir>/.pgui/connections.json
    let connections_file = home_dir.join(".pgui").join("connections.json");

    // Check if file exists
    if !connections_file.exists() {
        // File doesn't exist, nothing to load
        eprintln!("Connections file doesn't exist");
        return vec![];
    }

    let contents = fs::read_to_string(&connections_file).unwrap_or("[]".to_string());
    serde_json::from_str::<Vec<SavedConnection>>(&contents).unwrap_or(vec![])
}
