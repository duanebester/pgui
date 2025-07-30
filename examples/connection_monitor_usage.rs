/// Example of how to integrate the ConnectionMonitor into your GPUI application
/// 
/// This example shows how to:
/// 1. Create a ConnectionMonitor alongside your DatabaseManager
/// 2. Start/stop monitoring based on connection state
/// 3. Handle connection status updates in your UI
/// 4. Display connection status to users

use std::sync::Arc;
use std::time::Duration;
use gpui::*;
use async_std::task;

// Import your services
use crate::services::{DatabaseManager, ConnectionMonitor, ConnectionStatus, ConnectionEvent};

pub struct ConnectionsPanel {
    pub db_manager: Arc<DatabaseManager>,
    pub connection_monitor: ConnectionMonitor,
    connection_status: ConnectionStatus,
    last_ping_time: Option<std::time::SystemTime>,
    is_connected: bool,
    is_loading: bool,
    _subscriptions: Vec<Subscription>,
}

impl ConnectionsPanel {
    pub fn new(cx: &mut Window) -> Self {
        let db_manager = Arc::new(DatabaseManager::new());
        let connection_monitor = ConnectionMonitor::new(db_manager.clone())
            .with_ping_interval(Duration::from_secs(30)); // Ping every 30 seconds

        // Subscribe to connection status updates
        let status_receiver = connection_monitor.status_receiver();
        let entity = cx.entity();
        
        // Spawn a task to handle connection status updates
        task::spawn(async move {
            while let Ok(event) = status_receiver.recv().await {
                // Update the UI with the new connection status
                entity.update(|this: &mut Self, cx| {
                    this.handle_connection_event(event, cx);
                }).ok(); // Ignore errors if the entity is dropped
            }
        });

        Self {
            db_manager,
            connection_monitor,
            connection_status: ConnectionStatus::Disconnected,
            last_ping_time: None,
            is_connected: false,
            is_loading: false,
            _subscriptions: vec![],
        }
    }

    /// Handle connection status updates from the monitor
    fn handle_connection_event(&mut self, event: ConnectionEvent, cx: &mut ViewContext<Self>) {
        self.connection_status = event.status.clone();
        self.last_ping_time = Some(event.timestamp);

        match event.status {
            ConnectionStatus::Connected => {
                if !self.is_connected {
                    self.is_connected = true;
                    cx.notify(); // Trigger UI update
                    println!("✅ Database connection restored");
                }
            }
            ConnectionStatus::Disconnected => {
                if self.is_connected {
                    self.is_connected = false;
                    cx.notify(); // Trigger UI update
                    println!("❌ Database connection lost");
                }
            }
            ConnectionStatus::Error(error) => {
                if self.is_connected {
                    self.is_connected = false;
                    cx.notify(); // Trigger UI update
                }
                println!("🔥 Database connection error: {}", error);
            }
        }
    }

    /// Connect to database and start monitoring
    pub async fn connect_to_database(&mut self, database_url: &str) -> anyhow::Result<()> {
        self.is_loading = true;
        
        // Connect to the database
        match self.db_manager.connect(database_url).await {
            Ok(()) => {
                self.is_connected = true;
                self.is_loading = false;
                
                // Start monitoring the connection
                self.connection_monitor.start_monitoring().await?;
                println!("🚀 Connected to database and started monitoring");
                
                Ok(())
            }
            Err(e) => {
                self.is_connected = false;
                self.is_loading = false;
                Err(e)
            }
        }
    }

    /// Disconnect from database and stop monitoring
    pub async fn disconnect_from_database(&mut self) {
        // Stop monitoring first
        self.connection_monitor.stop_monitoring().await;
        
        // Then disconnect
        self.db_manager.disconnect().await;
        
        self.is_connected = false;
        self.connection_status = ConnectionStatus::Disconnected;
        self.last_ping_time = None;
        
        println!("📡 Disconnected from database and stopped monitoring");
    }

    /// Get current connection status for UI display
    pub fn get_connection_status_text(&self) -> String {
        match &self.connection_status {
            ConnectionStatus::Connected => {
                if let Some(last_ping) = self.last_ping_time {
                    if let Ok(elapsed) = last_ping.elapsed() {
                        format!("Connected (last ping: {}s ago)", elapsed.as_secs())
                    } else {
                        "Connected".to_string()
                    }
                } else {
                    "Connected".to_string()
                }
            }
            ConnectionStatus::Disconnected => "Disconnected".to_string(),
            ConnectionStatus::Error(error) => format!("Error: {}", error),
        }
    }

    /// Get connection status color for UI styling
    pub fn get_connection_status_color(&self) -> &str {
        match self.connection_status {
            ConnectionStatus::Connected => "green",
            ConnectionStatus::Disconnected => "red", 
            ConnectionStatus::Error(_) => "orange",
        }
    }

    /// Manually trigger a connection check (useful for testing)
    pub async fn check_connection_now(&self) -> ConnectionEvent {
        self.connection_monitor.check_connection_once().await
    }
}

// Example of more advanced monitoring with custom intervals and retry logic
pub struct AdvancedConnectionMonitor {
    monitor: ConnectionMonitor,
    retry_count: u32,
    max_retries: u32,
    backoff_duration: Duration,
}

impl AdvancedConnectionMonitor {
    pub fn new(db_manager: Arc<DatabaseManager>) -> Self {
        let monitor = ConnectionMonitor::new(db_manager)
            .with_ping_interval(Duration::from_secs(15)); // More frequent pings

        Self {
            monitor,
            retry_count: 0,
            max_retries: 3,
            backoff_duration: Duration::from_secs(5),
        }
    }

    /// Start monitoring with retry logic
    pub async fn start_with_retry(&mut self) -> anyhow::Result<()> {
        loop {
            match self.monitor.start_monitoring().await {
                Ok(()) => {
                    self.retry_count = 0;
                    return Ok(());
                }
                Err(e) => {
                    self.retry_count += 1;
                    if self.retry_count >= self.max_retries {
                        return Err(anyhow::anyhow!(
                            "Failed to start monitoring after {} retries: {}", 
                            self.max_retries, 
                            e
                        ));
                    }
                    
                    println!("Retrying monitoring start in {}s... (attempt {}/{})", 
                             self.backoff_duration.as_secs(), 
                             self.retry_count, 
                             self.max_retries);
                    
                    task::sleep(self.backoff_duration).await;
                }
            }
        }
    }
}

/// Example of how to create a dedicated connection status widget
pub struct ConnectionStatusWidget {
    status: ConnectionStatus,
    last_update: Option<std::time::SystemTime>,
}

impl ConnectionStatusWidget {
    pub fn new() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            last_update: None,
        }
    }

    pub fn update_status(&mut self, event: ConnectionEvent) {
        self.status = event.status;
        self.last_update = Some(event.timestamp);
    }
}

// Usage example in main application:
/*
pub async fn example_usage() {
    // Create database manager and connection monitor
    let db_manager = Arc::new(DatabaseManager::new());
    let monitor = ConnectionMonitor::new(db_manager.clone())
        .with_ping_interval(Duration::from_secs(30));

    // Connect to database
    db_manager.connect("postgresql://username:password@localhost/database").await?;

    // Start monitoring
    monitor.start_monitoring().await?;

    // Listen for status updates
    let status_receiver = monitor.status_receiver();
    task::spawn(async move {
        while let Ok(event) = status_receiver.recv().await {
            match event.status {
                ConnectionStatus::Connected => println!("✅ DB is healthy"),
                ConnectionStatus::Disconnected => println!("❌ DB connection lost"),
                ConnectionStatus::Error(e) => println!("🔥 DB error: {}", e),
            }
        }
    });

    // ... rest of your application logic
    
    // When shutting down:
    monitor.stop_monitoring().await;
    db_manager.disconnect().await;
}
*/