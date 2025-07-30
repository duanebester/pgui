use anyhow::Result;
use async_std::{task, stream::StreamExt, time::{Duration, interval}};
use async_channel::{Sender, Receiver, unbounded};
use std::sync::Arc;
use super::database::DatabaseManager;

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ConnectionEvent {
    pub status: ConnectionStatus,
    pub timestamp: std::time::SystemTime,
}

pub struct ConnectionMonitor {
    database_manager: Arc<DatabaseManager>,
    status_sender: Sender<ConnectionEvent>,
    status_receiver: Receiver<ConnectionEvent>,
    is_monitoring: Arc<async_std::sync::RwLock<bool>>,
    ping_interval: Duration,
}

impl ConnectionMonitor {
    pub fn new(database_manager: Arc<DatabaseManager>) -> Self {
        let (status_sender, status_receiver) = unbounded();
        
        Self {
            database_manager,
            status_sender,
            status_receiver,
            is_monitoring: Arc::new(async_std::sync::RwLock::new(false)),
            ping_interval: Duration::from_secs(30), // Default: ping every 30 seconds
        }
    }

    pub fn with_ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// Start the background monitoring task
    pub async fn start_monitoring(&self) -> Result<()> {
        let mut is_monitoring = self.is_monitoring.write().await;
        if *is_monitoring {
            return Ok(()); // Already monitoring
        }
        *is_monitoring = true;
        drop(is_monitoring);

        let database_manager = self.database_manager.clone();
        let status_sender = self.status_sender.clone();
        let is_monitoring = self.is_monitoring.clone();
        let ping_interval = self.ping_interval;

        // Spawn the monitoring task
        task::spawn(async move {
            let mut interval_stream = interval(ping_interval);
            
            while let Some(_) = interval_stream.next().await {
                // Check if we should continue monitoring
                {
                    let monitoring = is_monitoring.read().await;
                    if !*monitoring {
                        break;
                    }
                }

                // Perform the connection test
                let status = match database_manager.test_connection().await {
                    Ok(true) => ConnectionStatus::Connected,
                    Ok(false) => ConnectionStatus::Disconnected,
                    Err(e) => ConnectionStatus::Error(e.to_string()),
                };

                let event = ConnectionEvent {
                    status,
                    timestamp: std::time::SystemTime::now(),
                };

                // Send the status update (ignore if receiver is dropped)
                let _ = status_sender.send(event).await;
            }
        });

        Ok(())
    }

    /// Stop the background monitoring task
    pub async fn stop_monitoring(&self) {
        let mut is_monitoring = self.is_monitoring.write().await;
        *is_monitoring = false;
    }

    /// Get the receiver for connection status updates
    pub fn status_receiver(&self) -> Receiver<ConnectionEvent> {
        self.status_receiver.clone()
    }

    /// Check if currently monitoring
    pub async fn is_monitoring(&self) -> bool {
        *self.is_monitoring.read().await
    }

    /// Perform a one-time connection check
    pub async fn check_connection_once(&self) -> ConnectionEvent {
        let status = match self.database_manager.test_connection().await {
            Ok(true) => ConnectionStatus::Connected,
            Ok(false) => ConnectionStatus::Disconnected,
            Err(e) => ConnectionStatus::Error(e.to_string()),
        };

        ConnectionEvent {
            status,
            timestamp: std::time::SystemTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task;
    use std::time::Duration;

    #[async_std::test]
    async fn test_connection_monitor() {
        let db_manager = Arc::new(DatabaseManager::new());
        let monitor = ConnectionMonitor::new(db_manager)
            .with_ping_interval(Duration::from_millis(100));

        // Start monitoring
        monitor.start_monitoring().await.unwrap();
        assert!(monitor.is_monitoring().await);

        // Get a status update
        let receiver = monitor.status_receiver();
        let event = receiver.recv().await.unwrap();
        
        // Should be disconnected since we haven't connected to a DB
        match event.status {
            ConnectionStatus::Disconnected | ConnectionStatus::Error(_) => {},
            _ => panic!("Expected disconnected or error status"),
        }

        // Stop monitoring
        monitor.stop_monitoring().await;
        task::sleep(Duration::from_millis(200)).await;
        assert!(!monitor.is_monitoring().await);
    }
}