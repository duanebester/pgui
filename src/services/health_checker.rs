use anyhow::Result;
use async_std::{task, time::Duration};
use async_channel::{Sender, Receiver, unbounded};
use std::sync::Arc;
use super::database::DatabaseManager;

#[derive(Debug, Clone)]
pub struct HealthMetrics {
    pub is_healthy: bool,
    pub response_time_ms: u128,
    pub error_message: Option<String>,
    pub consecutive_failures: u32,
    pub last_success: Option<std::time::SystemTime>,
    pub last_failure: Option<std::time::SystemTime>,
    pub total_checks: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone)]
pub struct HealthCheckEvent {
    pub metrics: HealthMetrics,
    pub timestamp: std::time::SystemTime,
}

pub struct DatabaseHealthChecker {
    database_manager: Arc<DatabaseManager>,
    health_sender: Sender<HealthCheckEvent>,
    health_receiver: Receiver<HealthCheckEvent>,
    is_running: Arc<async_std::sync::RwLock<bool>>,
    
    // Configuration
    base_interval: Duration,
    max_interval: Duration,
    backoff_multiplier: f64,
    timeout: Duration,
    
    // State
    current_metrics: Arc<async_std::sync::RwLock<HealthMetrics>>,
}

impl DatabaseHealthChecker {
    pub fn new(database_manager: Arc<DatabaseManager>) -> Self {
        let (health_sender, health_receiver) = unbounded();
        
        let initial_metrics = HealthMetrics {
            is_healthy: false,
            response_time_ms: 0,
            error_message: None,
            consecutive_failures: 0,
            last_success: None,
            last_failure: None,
            total_checks: 0,
            success_rate: 0.0,
        };

        Self {
            database_manager,
            health_sender,
            health_receiver,
            is_running: Arc::new(async_std::sync::RwLock::new(false)),
            base_interval: Duration::from_secs(30),
            max_interval: Duration::from_secs(300), // 5 minutes max
            backoff_multiplier: 1.5,
            timeout: Duration::from_secs(10),
            current_metrics: Arc::new(async_std::sync::RwLock::new(initial_metrics)),
        }
    }

    pub fn with_intervals(mut self, base: Duration, max: Duration) -> Self {
        self.base_interval = base;
        self.max_interval = max;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        let database_manager = self.database_manager.clone();
        let health_sender = self.health_sender.clone();
        let is_running = self.is_running.clone();
        let current_metrics = self.current_metrics.clone();
        
        let base_interval = self.base_interval;
        let max_interval = self.max_interval;
        let backoff_multiplier = self.backoff_multiplier;
        let timeout = self.timeout;

        task::spawn(async move {
            let mut current_interval = base_interval;
            
            while {
                let running = is_running.read().await;
                *running
            } {
                let start_time = std::time::Instant::now();
                
                // Perform health check with timeout
                let health_result = task::timeout(timeout, async {
                    database_manager.test_connection().await
                }).await;

                let response_time = start_time.elapsed().as_millis();
                let now = std::time::SystemTime::now();

                // Update metrics
                let mut metrics = current_metrics.write().await;
                metrics.total_checks += 1;
                metrics.response_time_ms = response_time;

                let is_healthy = match health_result {
                    Ok(Ok(true)) => {
                        metrics.consecutive_failures = 0;
                        metrics.last_success = Some(now);
                        metrics.error_message = None;
                        current_interval = base_interval; // Reset interval on success
                        true
                    }
                    Ok(Ok(false)) => {
                        metrics.consecutive_failures += 1;
                        metrics.last_failure = Some(now);
                        metrics.error_message = Some("Connection test returned false".to_string());
                        false
                    }
                    Ok(Err(e)) => {
                        metrics.consecutive_failures += 1;
                        metrics.last_failure = Some(now);
                        metrics.error_message = Some(e.to_string());
                        false
                    }
                    Err(_) => {
                        metrics.consecutive_failures += 1;
                        metrics.last_failure = Some(now);
                        metrics.error_message = Some("Health check timed out".to_string());
                        false
                    }
                };

                metrics.is_healthy = is_healthy;
                
                // Calculate success rate
                let success_count = metrics.total_checks - metrics.consecutive_failures as u64;
                metrics.success_rate = (success_count as f64 / metrics.total_checks as f64) * 100.0;

                let event = HealthCheckEvent {
                    metrics: metrics.clone(),
                    timestamp: now,
                };
                drop(metrics);

                // Send health update
                let _ = health_sender.send(event).await;

                // Apply exponential backoff on failures
                if !is_healthy {
                    current_interval = Duration::from_millis(
                        (current_interval.as_millis() as f64 * backoff_multiplier) as u64
                    ).min(max_interval);
                }

                task::sleep(current_interval).await;
            }
        });

        Ok(())
    }

    pub async fn stop(&self) {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
    }

    pub fn health_receiver(&self) -> Receiver<HealthCheckEvent> {
        self.health_receiver.clone()
    }

    pub async fn get_current_metrics(&self) -> HealthMetrics {
        self.current_metrics.read().await.clone()
    }

    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Perform a one-time health check
    pub async fn check_once(&self) -> HealthCheckEvent {
        let start_time = std::time::Instant::now();
        let now = std::time::SystemTime::now();
        
        let health_result = task::timeout(self.timeout, async {
            self.database_manager.test_connection().await
        }).await;

        let response_time = start_time.elapsed().as_millis();

        let (is_healthy, error_message) = match health_result {
            Ok(Ok(true)) => (true, None),
            Ok(Ok(false)) => (false, Some("Connection test returned false".to_string())),
            Ok(Err(e)) => (false, Some(e.to_string())),
            Err(_) => (false, Some("Health check timed out".to_string())),
        };

        let metrics = HealthMetrics {
            is_healthy,
            response_time_ms: response_time,
            error_message,
            consecutive_failures: if is_healthy { 0 } else { 1 },
            last_success: if is_healthy { Some(now) } else { None },
            last_failure: if !is_healthy { Some(now) } else { None },
            total_checks: 1,
            success_rate: if is_healthy { 100.0 } else { 0.0 },
        };

        HealthCheckEvent {
            metrics,
            timestamp: now,
        }
    }
}

/// Utility functions for health monitoring
pub struct HealthMonitoringUtils;

impl HealthMonitoringUtils {
    /// Format health metrics for display
    pub fn format_health_status(metrics: &HealthMetrics) -> String {
        if metrics.is_healthy {
            format!(
                "✅ Healthy ({}ms) - {} checks, {:.1}% success rate",
                metrics.response_time_ms,
                metrics.total_checks,
                metrics.success_rate
            )
        } else {
            let failure_info = if metrics.consecutive_failures > 1 {
                format!(" - {} consecutive failures", metrics.consecutive_failures)
            } else {
                String::new()
            };

            format!(
                "❌ Unhealthy ({}ms){} - {}",
                metrics.response_time_ms,
                failure_info,
                metrics.error_message.as_deref().unwrap_or("Unknown error")
            )
        }
    }

    /// Get health status emoji
    pub fn get_status_emoji(metrics: &HealthMetrics) -> &'static str {
        if metrics.is_healthy {
            "✅"
        } else if metrics.consecutive_failures > 5 {
            "🔥" // Critical
        } else {
            "⚠️" // Warning
        }
    }

    /// Calculate uptime percentage
    pub fn calculate_uptime(events: &[HealthCheckEvent]) -> f64 {
        if events.is_empty() {
            return 0.0;
        }

        let healthy_count = events.iter()
            .filter(|event| event.metrics.is_healthy)
            .count();

        (healthy_count as f64 / events.len() as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[async_std::test]
    async fn test_health_checker() {
        let db_manager = Arc::new(DatabaseManager::new());
        let health_checker = DatabaseHealthChecker::new(db_manager)
            .with_intervals(Duration::from_millis(100), Duration::from_secs(1))
            .with_timeout(Duration::from_millis(500));

        // Start health checking
        health_checker.start().await.unwrap();
        assert!(health_checker.is_running().await);

        // Get a health event
        let receiver = health_checker.health_receiver();
        let event = receiver.recv().await.unwrap();
        
        // Should be unhealthy since we haven't connected to a DB
        assert!(!event.metrics.is_healthy);
        assert!(event.metrics.error_message.is_some());

        // Stop health checking
        health_checker.stop().await;
        task::sleep(Duration::from_millis(200)).await;
        assert!(!health_checker.is_running().await);
    }

    #[async_std::test]
    async fn test_health_metrics_formatting() {
        let metrics = HealthMetrics {
            is_healthy: true,
            response_time_ms: 42,
            error_message: None,
            consecutive_failures: 0,
            last_success: Some(std::time::SystemTime::now()),
            last_failure: None,
            total_checks: 100,
            success_rate: 95.0,
        };

        let status = HealthMonitoringUtils::format_health_status(&metrics);
        assert!(status.contains("✅"));
        assert!(status.contains("42ms"));
        assert!(status.contains("95.0%"));
    }
}