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

    /// Reset the backoff to initial state
    pub fn reset(&mut self) {
        self.current_delay = self.config.initial_delay;
        self.attempt = 0;
    }
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
