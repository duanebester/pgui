# Database Connection Monitoring

This project now includes two sophisticated database connection monitoring solutions:

## 1. ConnectionMonitor (Simple Approach)

A basic connection monitor that periodically pings the database and reports connection status.

### Features:
- ✅ Configurable ping intervals
- ✅ Async event-driven status updates
- ✅ Simple start/stop control
- ✅ One-time connection checks

### Usage:

```rust
use std::sync::Arc;
use std::time::Duration;
use crate::services::{DatabaseManager, ConnectionMonitor, ConnectionStatus};

// Create database manager and monitor
let db_manager = Arc::new(DatabaseManager::new());
let monitor = ConnectionMonitor::new(db_manager.clone())
    .with_ping_interval(Duration::from_secs(30));

// Connect to database
db_manager.connect("postgresql://user:pass@localhost/db").await?;

// Start monitoring
monitor.start_monitoring().await?;

// Listen for status updates
let receiver = monitor.status_receiver();
task::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        match event.status {
            ConnectionStatus::Connected => println!("✅ Database is healthy"),
            ConnectionStatus::Disconnected => println!("❌ Connection lost"),
            ConnectionStatus::Error(e) => println!("🔥 Error: {}", e),
        }
    }
});

// Stop monitoring when done
monitor.stop_monitoring().await;
```

## 2. DatabaseHealthChecker (Advanced Approach)

A comprehensive health monitoring system with detailed metrics, exponential backoff, and timeout handling.

### Features:
- ✅ Detailed health metrics and statistics
- ✅ Exponential backoff on failures
- ✅ Configurable timeouts
- ✅ Success rate tracking
- ✅ Response time monitoring
- ✅ Consecutive failure counting

### Usage:

```rust
use crate::services::{DatabaseManager, DatabaseHealthChecker, HealthMonitoringUtils};

// Create health checker
let db_manager = Arc::new(DatabaseManager::new());
let health_checker = DatabaseHealthChecker::new(db_manager.clone())
    .with_intervals(Duration::from_secs(15), Duration::from_secs(300))
    .with_timeout(Duration::from_secs(10));

// Start health monitoring
health_checker.start().await?;

// Listen for health events
let receiver = health_checker.health_receiver();
task::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        let status_text = HealthMonitoringUtils::format_health_status(&event.metrics);
        let emoji = HealthMonitoringUtils::get_status_emoji(&event.metrics);
        
        println!("{} {}", emoji, status_text);
        
        // You can access detailed metrics:
        println!("Response time: {}ms", event.metrics.response_time_ms);
        println!("Success rate: {:.1}%", event.metrics.success_rate);
        println!("Total checks: {}", event.metrics.total_checks);
        
        if let Some(error) = &event.metrics.error_message {
            println!("Last error: {}", error);
        }
    }
});

// Get current metrics at any time
let current_metrics = health_checker.get_current_metrics().await;
println!("Current health: {}", 
         HealthMonitoringUtils::format_health_status(&current_metrics));
```

## Integration with GPUI

For GPUI applications, you'll want to integrate the monitoring into your UI components. Here's how:

### In your ConnectionsPanel:

```rust
pub struct ConnectionsPanel {
    pub db_manager: Arc<DatabaseManager>,
    pub connection_monitor: ConnectionMonitor,
    connection_status: ConnectionStatus,
    // ... other fields
}

impl ConnectionsPanel {
    pub fn new(cx: &mut Window) -> Self {
        let db_manager = Arc::new(DatabaseManager::new());
        let monitor = ConnectionMonitor::new(db_manager.clone())
            .with_ping_interval(Duration::from_secs(30));

        // Subscribe to status updates
        let status_receiver = monitor.status_receiver();
        let entity = cx.entity();
        
        task::spawn(async move {
            while let Ok(event) = status_receiver.recv().await {
                entity.update(|this: &mut Self, cx| {
                    this.handle_connection_event(event, cx);
                }).ok();
            }
        });

        Self {
            db_manager,
            connection_monitor: monitor,
            // ... initialize other fields
        }
    }

    fn handle_connection_event(&mut self, event: ConnectionEvent, cx: &mut ViewContext<Self>) {
        self.connection_status = event.status;
        cx.notify(); // Trigger UI refresh
    }
}
```

## Configuration Options

### ConnectionMonitor
- `ping_interval`: How often to check the connection (default: 30s)

### DatabaseHealthChecker
- `base_interval`: Initial check interval (default: 30s)
- `max_interval`: Maximum interval during backoff (default: 5 minutes)
- `backoff_multiplier`: How much to increase interval on failures (default: 1.5x)
- `timeout`: Maximum time to wait for a health check (default: 10s)

## Best Practices

1. **Choose the Right Monitor**: Use `ConnectionMonitor` for simple needs, `DatabaseHealthChecker` for production applications requiring detailed metrics.

2. **Set Appropriate Intervals**: Don't ping too frequently (avoid overwhelming the database), but often enough to detect issues quickly.

3. **Handle UI Updates**: Always use `cx.notify()` in GPUI when updating connection status to trigger re-renders.

4. **Graceful Shutdown**: Always stop monitoring when your application shuts down:
   ```rust
   monitor.stop_monitoring().await;
   db_manager.disconnect().await;
   ```

5. **Error Handling**: Handle connection status changes appropriately in your UI - disable features when disconnected, show retry options, etc.

6. **Resource Management**: Both monitors use channels and background tasks. They'll clean up automatically when dropped, but explicit cleanup is recommended.

## Example UI Integration

```rust
// In your UI rendering code
fn render_connection_status(&self) -> impl IntoElement {
    let status_text = match &self.connection_status {
        ConnectionStatus::Connected => "🟢 Connected",
        ConnectionStatus::Disconnected => "🔴 Disconnected", 
        ConnectionStatus::Error(e) => &format!("🟡 Error: {}", e),
    };
    
    div()
        .child(Label::new(status_text))
        .when(matches!(self.connection_status, ConnectionStatus::Error(_)), |el| {
            el.child(
                Button::new("retry")
                    .label("Retry Connection")
                    .on_click(|this: &mut Self, cx| {
                        // Trigger reconnection
                        this.reconnect(cx);
                    })
            )
        })
}
```

## Testing

Both monitoring services include comprehensive tests. Run them with:

```bash
cargo test connection_monitor
cargo test health_checker
```

The monitors are designed to work even when no database is connected - they'll report disconnected/error status, which makes them easy to test and develop with.