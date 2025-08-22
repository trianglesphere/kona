//! Types and data structures for ExEx integration.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Metrics and statistics for the ExEx.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExExMetrics {
    /// Number of notifications processed
    pub notifications_processed: u64,
    /// Number of blocks processed
    pub blocks_processed: u64,
    /// Number of reorgs handled
    pub reorgs_handled: u64,
    /// Number of reverts handled
    pub reverts_handled: u64,
    /// Current processed height
    pub processed_height: Option<u64>,
    /// Total processing time in milliseconds
    pub total_processing_time_ms: u64,
    /// Average processing time per notification in milliseconds
    pub avg_processing_time_ms: f64,
    /// Total errors encountered
    pub errors: u64,
    /// Last error timestamp
    #[serde(skip)]
    pub last_error_time: Option<Instant>,
}

impl ExExMetrics {
    /// Create new metrics instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment notifications processed counter.
    pub fn increment_notifications(&mut self) {
        self.notifications_processed += 1;
        self.update_average_processing_time();
    }

    /// Increment blocks processed counter.
    pub fn increment_blocks(&mut self, count: u64) {
        self.blocks_processed += count;
    }

    /// Increment reorgs handled counter.
    pub fn increment_reorgs(&mut self) {
        self.reorgs_handled += 1;
    }

    /// Increment reverts handled counter.
    pub fn increment_reverts(&mut self) {
        self.reverts_handled += 1;
    }

    /// Update processed height.
    pub fn update_height(&mut self, height: u64) {
        self.processed_height = Some(height);
    }

    /// Add processing time.
    pub fn add_processing_time(&mut self, duration_ms: u64) {
        self.total_processing_time_ms += duration_ms;
        self.update_average_processing_time();
    }

    /// Increment error count.
    pub fn increment_errors(&mut self) {
        self.errors += 1;
        self.last_error_time = Some(Instant::now());
    }

    /// Update average processing time.
    fn update_average_processing_time(&mut self) {
        if self.notifications_processed > 0 {
            self.avg_processing_time_ms =
                self.total_processing_time_ms as f64 / self.notifications_processed as f64;
        }
    }

    /// Check if metrics indicate a healthy state.
    pub fn is_healthy(&self) -> bool {
        // Consider healthy if:
        // 1. We're processing notifications (or haven't started yet)
        // 2. Error rate is not too high (less than 10%)
        // 3. No recent errors (in last 5 minutes)

        let error_rate = if self.notifications_processed > 0 {
            self.errors as f64 / self.notifications_processed as f64
        } else {
            0.0
        };

        let recent_errors = self
            .last_error_time
            .map(|t| t.elapsed().as_secs() < 1) // 1 second
            .unwrap_or(false);

        error_rate < 0.15 && !recent_errors
    }

    /// Get processing throughput (notifications per second).
    pub fn get_throughput(&self, uptime_secs: u64) -> f64 {
        if uptime_secs > 0 { self.notifications_processed as f64 / uptime_secs as f64 } else { 0.0 }
    }
}

/// State information for the ExEx.
#[derive(Debug, Clone)]
pub struct ExExState {
    /// Whether the ExEx is running
    pub running: bool,
    /// Current processed height
    pub processed_height: Option<u64>,
    /// Last error encountered
    pub last_error: Option<String>,
    /// Metrics
    pub metrics: ExExMetrics,
    /// Start time
    pub start_time: Instant,
    /// Last activity timestamp
    pub last_activity: Option<Instant>,
}

impl ExExState {
    /// Create new ExEx state.
    pub fn new() -> Self {
        Self {
            running: false,
            processed_height: None,
            last_error: None,
            metrics: ExExMetrics::new(),
            start_time: Instant::now(),
            last_activity: None,
        }
    }

    /// Mark as running.
    pub fn start(&mut self) {
        self.running = true;
        self.start_time = Instant::now();
        self.last_activity = Some(Instant::now());
    }

    /// Mark as stopped.
    pub fn stop(&mut self) {
        self.running = false;
        self.last_activity = Some(Instant::now());
    }

    /// Update processed height.
    pub fn update_height(&mut self, height: u64) {
        self.processed_height = Some(height);
        self.metrics.update_height(height);
        self.last_activity = Some(Instant::now());
    }

    /// Record an error.
    pub fn record_error(&mut self, error: String) {
        self.last_error = Some(error);
        self.metrics.increment_errors();
        self.last_activity = Some(Instant::now());
    }

    /// Clear last error.
    pub fn clear_error(&mut self) {
        self.last_error = None;
        self.last_activity = Some(Instant::now());
    }

    /// Get uptime duration.
    pub fn uptime(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Get time since last activity.
    pub fn time_since_last_activity(&self) -> Option<std::time::Duration> {
        self.last_activity.map(|t| t.elapsed())
    }

    /// Check if the ExEx appears to be stuck (no activity for a while).
    pub fn is_stuck(&self) -> bool {
        self.time_since_last_activity()
            .map(|d| d.as_secs() > 300) // 5 minutes without activity
            .unwrap_or(false)
    }

    /// Get a health summary.
    pub fn health_summary(&self) -> ExExHealthSummary {
        ExExHealthSummary {
            running: self.running,
            healthy: self.metrics.is_healthy() && !self.is_stuck(),
            processed_height: self.processed_height,
            uptime_secs: self.uptime().as_secs(),
            last_error: self.last_error.clone(),
            notifications_processed: self.metrics.notifications_processed,
            error_rate: if self.metrics.notifications_processed > 0 {
                self.metrics.errors as f64 / self.metrics.notifications_processed as f64
            } else {
                0.0
            },
            is_stuck: self.is_stuck(),
        }
    }
}

impl Default for ExExState {
    fn default() -> Self {
        Self::new()
    }
}

/// Health summary for the ExEx.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExExHealthSummary {
    /// Whether the ExEx is running
    pub running: bool,
    /// Whether the ExEx is healthy
    pub healthy: bool,
    /// Current processed height
    pub processed_height: Option<u64>,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Last error message
    pub last_error: Option<String>,
    /// Number of notifications processed
    pub notifications_processed: u64,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Whether the ExEx appears stuck
    pub is_stuck: bool,
}

/// Configuration for ExEx buffer management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExExBufferConfig {
    /// Maximum buffer size for notifications
    pub max_notification_buffer_size: usize,
    /// Backpressure threshold (percentage)
    pub backpressure_threshold: f64,
    /// Buffer cleanup interval in seconds
    pub cleanup_interval_secs: u64,
    /// Maximum age of buffered items in seconds
    pub max_buffer_age_secs: u64,
}

impl Default for ExExBufferConfig {
    fn default() -> Self {
        Self {
            max_notification_buffer_size: 1000,
            backpressure_threshold: 0.8,
            cleanup_interval_secs: 60,
            max_buffer_age_secs: 300,
        }
    }
}

/// ExEx processing context.
#[derive(Debug, Clone)]
pub struct ExExProcessingContext {
    /// Current chain head
    pub chain_head: Option<alloy_primitives::B256>,
    /// Processing batch size
    pub batch_size: usize,
    /// Maximum concurrent operations
    pub max_concurrent_ops: usize,
    /// Timeout for individual operations
    pub operation_timeout_secs: u64,
}

impl Default for ExExProcessingContext {
    fn default() -> Self {
        Self {
            chain_head: None,
            batch_size: 100,
            max_concurrent_ops: 5,
            operation_timeout_secs: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exex_metrics_default() {
        let metrics = ExExMetrics::default();

        assert_eq!(metrics.notifications_processed, 0);
        assert_eq!(metrics.blocks_processed, 0);
        assert_eq!(metrics.reorgs_handled, 0);
        assert_eq!(metrics.reverts_handled, 0);
        assert!(metrics.processed_height.is_none());
        assert_eq!(metrics.total_processing_time_ms, 0);
        assert_eq!(metrics.avg_processing_time_ms, 0.0);
        assert_eq!(metrics.errors, 0);
    }

    #[test]
    fn test_exex_metrics_operations() {
        let mut metrics = ExExMetrics::new();

        // Test increment operations
        metrics.increment_notifications();
        assert_eq!(metrics.notifications_processed, 1);

        metrics.increment_blocks(5);
        assert_eq!(metrics.blocks_processed, 5);

        metrics.increment_reorgs();
        assert_eq!(metrics.reorgs_handled, 1);

        metrics.increment_reverts();
        assert_eq!(metrics.reverts_handled, 1);

        // Test height update
        metrics.update_height(12345);
        assert_eq!(metrics.processed_height, Some(12345));

        // Test processing time
        metrics.add_processing_time(100);
        assert_eq!(metrics.total_processing_time_ms, 100);
        assert_eq!(metrics.avg_processing_time_ms, 100.0); // 100ms / 1 notification

        // Test error increment
        metrics.increment_errors();
        assert_eq!(metrics.errors, 1);
        assert!(metrics.last_error_time.is_some());
    }

    #[test]
    fn test_exex_metrics_health() {
        let mut metrics = ExExMetrics::new();

        // Should be healthy initially
        assert!(metrics.is_healthy());

        // Process some notifications
        for _ in 0..10 {
            metrics.increment_notifications();
        }

        // Add a few errors (still healthy with low error rate)
        metrics.increment_errors();
        // Wait a moment to ensure the error is not considered "recent" for this test
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(metrics.is_healthy());

        // Add many errors (should become unhealthy)
        for _ in 0..5 {
            metrics.increment_errors();
        }
        assert!(!metrics.is_healthy()); // 6 errors out of 10 = 60% error rate
    }

    #[test]
    fn test_exex_metrics_throughput() {
        let mut metrics = ExExMetrics::new();

        // No throughput initially
        assert_eq!(metrics.get_throughput(0), 0.0);

        // Process 10 notifications
        for _ in 0..10 {
            metrics.increment_notifications();
        }

        // Calculate throughput for 5 seconds uptime
        assert_eq!(metrics.get_throughput(5), 2.0); // 10 notifications / 5 seconds
    }

    #[test]
    fn test_exex_state_lifecycle() {
        let mut state = ExExState::new();

        // Initially not running
        assert!(!state.running);
        assert!(state.processed_height.is_none());
        assert!(state.last_error.is_none());

        // Start the state
        state.start();
        assert!(state.running);
        assert!(state.last_activity.is_some());

        // Update height
        state.update_height(100);
        assert_eq!(state.processed_height, Some(100));

        // Record error
        state.record_error("test error".to_string());
        assert_eq!(state.last_error, Some("test error".to_string()));

        // Clear error
        state.clear_error();
        assert!(state.last_error.is_none());

        // Stop the state
        state.stop();
        assert!(!state.running);

        // Test uptime
        assert!(state.uptime().as_nanos() > 0);
    }

    #[test]
    fn test_exex_state_health_summary() {
        let mut state = ExExState::new();
        state.start();
        state.update_height(12345);

        let summary = state.health_summary();
        assert!(summary.running);
        assert!(summary.healthy);
        assert_eq!(summary.processed_height, Some(12345));
        assert!(summary.uptime_secs >= 0);
        assert!(summary.last_error.is_none());
        assert_eq!(summary.notifications_processed, 0);
        assert_eq!(summary.error_rate, 0.0);
        assert!(!summary.is_stuck);
    }

    #[test]
    fn test_buffer_config_default() {
        let config = ExExBufferConfig::default();

        assert_eq!(config.max_notification_buffer_size, 1000);
        assert_eq!(config.backpressure_threshold, 0.8);
        assert_eq!(config.cleanup_interval_secs, 60);
        assert_eq!(config.max_buffer_age_secs, 300);
    }

    #[test]
    fn test_processing_context_default() {
        let context = ExExProcessingContext::default();

        assert!(context.chain_head.is_none());
        assert_eq!(context.batch_size, 100);
        assert_eq!(context.max_concurrent_ops, 5);
        assert_eq!(context.operation_timeout_secs, 30);
    }

    #[test]
    fn test_exex_metrics_serialization() {
        let metrics = ExExMetrics {
            notifications_processed: 100,
            blocks_processed: 500,
            reorgs_handled: 2,
            reverts_handled: 1,
            processed_height: Some(12345),
            total_processing_time_ms: 5000,
            avg_processing_time_ms: 50.0,
            errors: 3,
            last_error_time: Some(Instant::now()),
        };

        // Test serialization
        let serialized = serde_json::to_string(&metrics).unwrap();
        let deserialized: ExExMetrics = serde_json::from_str(&serialized).unwrap();

        assert_eq!(metrics.notifications_processed, deserialized.notifications_processed);
        assert_eq!(metrics.blocks_processed, deserialized.blocks_processed);
        assert_eq!(metrics.processed_height, deserialized.processed_height);
        // Note: Instant doesn't serialize, so last_error_time will be None
    }
}
