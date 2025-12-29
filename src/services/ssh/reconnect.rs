//! Automatic reconnection with exponential backoff.

use std::time::Duration;

/// Configuration for reconnection behavior
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub multiplier: f64,
    /// Maximum number of retry attempts (None = infinite)
    pub max_attempts: Option<u32>,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            max_attempts: Some(5),
        }
    }
}

/// Exponential backoff iterator for reconnection attempts
pub struct ExponentialBackoff {
    config: ReconnectConfig,
    current_delay: Duration,
    attempt: u32,
}

impl ExponentialBackoff {
    /// Create a new exponential backoff with the given config
    pub fn new(config: ReconnectConfig) -> Self {
        Self {
            current_delay: config.initial_delay,
            attempt: 0,
            config,
        }
    }

    /// Get the next delay, or None if max attempts reached
    pub fn next_delay(&mut self) -> Option<Duration> {
        if let Some(max) = self.config.max_attempts {
            if self.attempt >= max {
                return None;
            }
        }

        let delay = self.current_delay;
        self.attempt += 1;

        // Calculate next delay with exponential backoff
        let next =
            Duration::from_secs_f64(self.current_delay.as_secs_f64() * self.config.multiplier);
        self.current_delay = next.min(self.config.max_delay);

        Some(delay)
    }

    /// Get the current attempt number (1-based after first call to next_delay)
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Get the maximum number of attempts (or u32::MAX if unlimited)
    pub fn max_attempts(&self) -> u32 {
        self.config.max_attempts.unwrap_or(u32::MAX)
    }

    /// Reset the backoff to initial state
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.current_delay = self.config.initial_delay;
        self.attempt = 0;
    }
}

/// Determine if an SSH tunnel error is retriable.
///
/// Non-retriable errors (return false):
/// - Authentication failures (wrong password, key rejected)
/// - Permission denied
/// - Host key verification failures
///
/// Retriable errors (return true):
/// - Connection refused (server temporarily down)
/// - Connection timed out (network issues)
/// - Port binding issues (race condition)
/// - Network unreachable
pub fn is_retriable_error(error: &anyhow::Error) -> bool {
    let error_str = error.to_string().to_lowercase();

    // Non-retriable: authentication and permission errors
    let non_retriable_patterns = [
        "permission denied",
        "authentication failed",
        "auth fail",
        "host key verification failed",
        "no supported authentication",
        "too many authentication failures",
        "invalid password",
        "key rejected",
        "publickey denied",
    ];

    for pattern in &non_retriable_patterns {
        if error_str.contains(pattern) {
            return false;
        }
    }

    // Retriable: network and transient errors
    let retriable_patterns = [
        "connection refused",
        "connection timed out",
        "connection reset",
        "network unreachable",
        "host unreachable",
        "no route to host",
        "not listening",
        "address already in use",
        "temporary failure",
        "try again",
    ];

    for pattern in &retriable_patterns {
        if error_str.contains(pattern) {
            return true;
        }
    }

    // Default: don't retry unknown errors (fail fast)
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            max_attempts: Some(4),
        };

        let mut backoff = ExponentialBackoff::new(config);

        // First delay: 1s
        assert_eq!(backoff.next_delay(), Some(Duration::from_secs(1)));
        assert_eq!(backoff.attempt(), 1);

        // Second delay: 2s
        assert_eq!(backoff.next_delay(), Some(Duration::from_secs(2)));
        assert_eq!(backoff.attempt(), 2);

        // Third delay: 4s
        assert_eq!(backoff.next_delay(), Some(Duration::from_secs(4)));
        assert_eq!(backoff.attempt(), 3);

        // Fourth delay: 8s
        assert_eq!(backoff.next_delay(), Some(Duration::from_secs(8)));
        assert_eq!(backoff.attempt(), 4);

        // Fifth attempt: None (max reached)
        assert_eq!(backoff.next_delay(), None);

        // Reset and verify
        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
        assert_eq!(backoff.next_delay(), Some(Duration::from_secs(1)));

        // Test max_attempts helper
        assert_eq!(backoff.max_attempts(), 4);
    }

    #[test]
    fn test_unlimited_attempts() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            max_attempts: None, // Unlimited
        };

        let backoff = ExponentialBackoff::new(config);
        assert_eq!(backoff.max_attempts(), u32::MAX);
    }

    #[test]
    fn test_is_retriable_error() {
        // Non-retriable errors
        let auth_error = anyhow::anyhow!("Permission denied (publickey,password)");
        assert!(!is_retriable_error(&auth_error));

        let auth_error2 = anyhow::anyhow!("Authentication failed for user@host");
        assert!(!is_retriable_error(&auth_error2));

        let host_key_error = anyhow::anyhow!("Host key verification failed");
        assert!(!is_retriable_error(&host_key_error));

        // Retriable errors
        let connection_refused = anyhow::anyhow!("Connection refused by host");
        assert!(is_retriable_error(&connection_refused));

        let timeout = anyhow::anyhow!("Connection timed out");
        assert!(is_retriable_error(&timeout));

        let port_error = anyhow::anyhow!("SSH tunnel failed - local port 12345 not listening");
        assert!(is_retriable_error(&port_error));

        let network_error = anyhow::anyhow!("Network unreachable");
        assert!(is_retriable_error(&network_error));

        // Unknown errors default to non-retriable
        let unknown = anyhow::anyhow!("Something weird happened");
        assert!(!is_retriable_error(&unknown));
    }

    #[test]
    fn test_max_delay_cap() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(10),
            multiplier: 3.0,
            max_attempts: Some(3),
        };

        let mut backoff = ExponentialBackoff::new(config);

        // First: 5s
        assert_eq!(backoff.next_delay(), Some(Duration::from_secs(5)));
        // Second: would be 15s, capped to 10s
        assert_eq!(backoff.next_delay(), Some(Duration::from_secs(10)));
        // Third: still 10s (capped)
        assert_eq!(backoff.next_delay(), Some(Duration::from_secs(10)));
    }
}
