//! Network monitoring and metrics collection for scoring examples
//!
//! This module provides monitoring capabilities to track network performance,
//! peer behavior, and scoring effectiveness in real-time.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use crate::config::ScoringConfig;

/// Network monitoring and metrics collector.
#[derive(Debug)]
pub struct NetworkMonitor {
    config: ScoringConfig,
    stats: Arc<Mutex<NetworkStats>>,
    start_time: Instant,
}

/// Network statistics and metrics.
#[derive(Debug, Default)]
pub struct NetworkStats {
    /// Total blocks received
    pub blocks_received: u64,
    /// Blocks received in the last period
    pub recent_blocks: u64,
    /// Last stats reset time
    #[allow(dead_code)]
    pub last_reset: Option<Instant>,
    /// Peer connection events
    pub peer_events: PeerEventStats,
    /// Message propagation metrics
    pub message_stats: MessageStats,
}

/// Peer-related event statistics.
#[derive(Debug, Default)]
pub struct PeerEventStats {
    /// Total peer connections
    pub total_connections: u64,
    /// Total peer disconnections
    pub total_disconnections: u64,
    /// Currently connected peers
    pub current_peers: u64,
    /// Banned peers count
    pub banned_peers: u64,
    /// Peer ban events
    pub ban_events: Vec<PeerBanEvent>,
}

/// Message propagation and validation statistics.
#[derive(Debug, Default)]
pub struct MessageStats {
    /// Total messages received
    pub messages_received: u64,
    /// Valid messages
    pub valid_messages: u64,
    /// Invalid messages
    #[allow(dead_code)]
    pub invalid_messages: u64,
    /// Duplicate messages
    pub duplicate_messages: u64,
    /// Average message latency
    #[allow(dead_code)]
    pub avg_latency: Option<Duration>,
    /// Message propagation by topic
    #[allow(dead_code)]
    pub topic_stats: HashMap<String, TopicStats>,
}

/// Statistics for a specific topic.
#[derive(Debug, Default)]
pub struct TopicStats {
    /// Messages on this topic
    #[allow(dead_code)]
    pub message_count: u64,
    /// Unique senders
    #[allow(dead_code)]
    pub unique_senders: u64,
    /// Average message size
    #[allow(dead_code)]
    pub avg_message_size: u64,
}

/// Record of a peer ban event.
#[derive(Debug, Clone)]
pub struct PeerBanEvent {
    /// Peer identifier
    pub peer_id: String,
    /// Ban timestamp
    pub timestamp: Instant,
    /// Score that triggered the ban
    pub score: f64,
    /// Reason for the ban
    pub reason: String,
}

impl NetworkMonitor {
    /// Create a new network monitor.
    pub fn new(config: ScoringConfig) -> Self {
        Self {
            config,
            stats: Arc::new(Mutex::new(NetworkStats::default())),
            start_time: Instant::now(),
        }
    }

    /// Record a block being received.
    pub async fn record_block_received(&self) {
        let mut stats = self.stats.lock().await;
        stats.blocks_received += 1;
        stats.recent_blocks += 1;
    }

    /// Record a peer connection event.
    #[allow(dead_code)]
    pub async fn record_peer_connected(&self, peer_id: String) {
        let mut stats = self.stats.lock().await;
        stats.peer_events.total_connections += 1;
        stats.peer_events.current_peers += 1;

        tracing::debug!(
            target: "monitoring",
            "Peer connected: {} (total: {})",
            peer_id,
            stats.peer_events.current_peers
        );
    }

    /// Record a peer disconnection event.
    #[allow(dead_code)]
    pub async fn record_peer_disconnected(&self, peer_id: String) {
        let mut stats = self.stats.lock().await;
        stats.peer_events.total_disconnections += 1;
        if stats.peer_events.current_peers > 0 {
            stats.peer_events.current_peers -= 1;
        }

        tracing::debug!(
            target: "monitoring",
            "Peer disconnected: {} (remaining: {})",
            peer_id,
            stats.peer_events.current_peers
        );
    }

    /// Record a peer ban event.
    #[allow(dead_code)]
    pub async fn record_peer_banned(&self, peer_id: String, score: f64, reason: String) {
        let mut stats = self.stats.lock().await;
        stats.peer_events.banned_peers += 1;

        let ban_event = PeerBanEvent {
            peer_id: peer_id.clone(),
            timestamp: Instant::now(),
            score,
            reason: reason.clone(),
        };

        stats.peer_events.ban_events.push(ban_event);

        tracing::warn!(
            target: "monitoring",
            "Peer banned: {} (score: {}, reason: {})",
            peer_id,
            score,
            reason
        );
    }

    /// Record a message being received.
    #[allow(dead_code)]
    pub async fn record_message_received(
        &self,
        topic: String,
        size: usize,
        valid: bool,
        duplicate: bool,
    ) {
        let mut stats = self.stats.lock().await;
        stats.message_stats.messages_received += 1;

        if valid {
            stats.message_stats.valid_messages += 1;
        } else {
            stats.message_stats.invalid_messages += 1;
        }

        if duplicate {
            stats.message_stats.duplicate_messages += 1;
        }

        // Update topic-specific stats
        let topic_stat = stats.message_stats.topic_stats.entry(topic).or_default();
        topic_stat.message_count += 1;

        // Update average message size (simple moving average)
        if topic_stat.message_count == 1 {
            topic_stat.avg_message_size = size as u64;
        } else {
            topic_stat.avg_message_size =
                (topic_stat.avg_message_size * (topic_stat.message_count - 1) + size as u64) /
                    topic_stat.message_count;
        }
    }

    /// Log comprehensive network statistics.
    pub async fn log_network_stats(&self) {
        let stats = self.stats.lock().await;
        let uptime = self.start_time.elapsed();

        tracing::info!(
            target: "monitoring",
            "=== Network Statistics (uptime: {:?}) ===",
            uptime
        );

        // Block statistics
        tracing::info!(
            target: "monitoring",
            "Blocks: {} total, {} recent (rate: {:.2}/min)",
            stats.blocks_received,
            stats.recent_blocks,
            stats.recent_blocks as f64 / uptime.as_secs_f64() * 60.0
        );

        // Peer statistics
        tracing::info!(
            target: "monitoring",
            "Peers: {} current, {} total connections, {} disconnections, {} banned",
            stats.peer_events.current_peers,
            stats.peer_events.total_connections,
            stats.peer_events.total_disconnections,
            stats.peer_events.banned_peers
        );

        // Message statistics
        if stats.message_stats.messages_received > 0 {
            let validity_rate = stats.message_stats.valid_messages as f64 /
                stats.message_stats.messages_received as f64 *
                100.0;
            let duplicate_rate = stats.message_stats.duplicate_messages as f64 /
                stats.message_stats.messages_received as f64 *
                100.0;

            tracing::info!(
                target: "monitoring",
                "Messages: {} total, {:.1}% valid, {:.1}% duplicates",
                stats.message_stats.messages_received,
                validity_rate,
                duplicate_rate
            );
        }

        // Recent ban events
        let recent_bans: Vec<&PeerBanEvent> = stats
            .peer_events
            .ban_events
            .iter()
            .filter(|event| event.timestamp.elapsed() < Duration::from_secs(300)) // Last 5 minutes
            .collect();

        if !recent_bans.is_empty() {
            tracing::warn!(
                target: "monitoring",
                "Recent bans ({} in last 5 minutes):",
                recent_bans.len()
            );
            for ban in recent_bans.iter().take(5) {
                // Show up to 5 recent bans
                tracing::warn!(
                    target: "monitoring",
                    "  {} (score: {}, reason: {})",
                    ban.peer_id,
                    ban.score,
                    ban.reason
                );
            }
        }

        // Scoring configuration reminder
        tracing::info!(
            target: "monitoring",
            "Config: profile={:?}, ban_threshold={}, topic_scoring={}",
            self.config.profile,
            self.config.ban_threshold,
            self.config.topic_scoring
        );
    }

    /// Get current network health score (0.0 to 1.0).
    #[allow(dead_code)]
    pub async fn network_health_score(&self) -> f64 {
        let stats = self.stats.lock().await;
        let mut health_factors = Vec::new();

        // Peer connectivity factor
        let peer_factor = if stats.peer_events.current_peers >= 4 {
            1.0
        } else {
            stats.peer_events.current_peers as f64 / 4.0
        };
        health_factors.push(peer_factor);

        // Message validity factor
        if stats.message_stats.messages_received > 0 {
            let validity_factor = stats.message_stats.valid_messages as f64 /
                stats.message_stats.messages_received as f64;
            health_factors.push(validity_factor);
        }

        // Ban rate factor (lower ban rate = better health)
        let uptime_minutes = self.start_time.elapsed().as_secs_f64() / 60.0;
        let ban_rate = if uptime_minutes > 0.0 {
            stats.peer_events.banned_peers as f64 / uptime_minutes
        } else {
            0.0
        };
        let ban_factor = if ban_rate > 2.0 { 0.5 } else { 1.0 - (ban_rate / 4.0) };
        health_factors.push(ban_factor);

        // Calculate average health score
        if health_factors.is_empty() {
            0.5 // Neutral health if no data
        } else {
            health_factors.iter().sum::<f64>() / health_factors.len() as f64
        }
    }

    /// Reset recent statistics (useful for periodic reporting).
    #[allow(dead_code)]
    pub async fn reset_recent_stats(&self) {
        let mut stats = self.stats.lock().await;
        stats.recent_blocks = 0;
        stats.last_reset = Some(Instant::now());
    }

    /// Export statistics as JSON for external monitoring.
    #[allow(dead_code)]
    pub async fn export_json(&self) -> Result<String, serde_json::Error> {
        let stats = self.stats.lock().await;
        let export_data = serde_json::json!({
            "uptime_seconds": self.start_time.elapsed().as_secs(),
            "blocks_received": stats.blocks_received,
            "current_peers": stats.peer_events.current_peers,
            "total_connections": stats.peer_events.total_connections,
            "banned_peers": stats.peer_events.banned_peers,
            "messages_received": stats.message_stats.messages_received,
            "valid_messages": stats.message_stats.valid_messages,
            "invalid_messages": stats.message_stats.invalid_messages,
            "duplicate_messages": stats.message_stats.duplicate_messages,
            "network_health": self.network_health_score().await,
            "config_profile": self.config.profile,
            "ban_threshold": self.config.ban_threshold,
            "topic_scoring": self.config.topic_scoring
        });
        serde_json::to_string_pretty(&export_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::ScoringProfile;

    #[tokio::test]
    async fn test_monitor_basic_operations() {
        let config =
            ScoringConfig::new(ScoringProfile::Conservative, false, None, None, 2).unwrap();

        let monitor = NetworkMonitor::new(config);

        // Test block recording
        monitor.record_block_received().await;
        monitor.record_block_received().await;

        let stats = monitor.stats.lock().await;
        assert_eq!(stats.blocks_received, 2);
        assert_eq!(stats.recent_blocks, 2);
    }

    #[tokio::test]
    async fn test_peer_event_recording() {
        let config = ScoringConfig::new(ScoringProfile::Moderate, true, None, None, 2).unwrap();

        let monitor = NetworkMonitor::new(config);

        // Test peer events
        monitor.record_peer_connected("peer1".to_string()).await;
        monitor.record_peer_connected("peer2".to_string()).await;
        monitor.record_peer_disconnected("peer1".to_string()).await;
        monitor.record_peer_banned("peer3".to_string(), -50.0, "Low score".to_string()).await;

        let stats = monitor.stats.lock().await;
        assert_eq!(stats.peer_events.total_connections, 2);
        assert_eq!(stats.peer_events.total_disconnections, 1);
        assert_eq!(stats.peer_events.current_peers, 1);
        assert_eq!(stats.peer_events.banned_peers, 1);
        assert_eq!(stats.peer_events.ban_events.len(), 1);
        assert_eq!(stats.peer_events.ban_events[0].peer_id, "peer3");
    }

    #[tokio::test]
    async fn test_network_health_calculation() {
        let config =
            ScoringConfig::new(ScoringProfile::Conservative, false, None, None, 2).unwrap();

        let monitor = NetworkMonitor::new(config);

        // Initial health should be neutral
        let initial_health = monitor.network_health_score().await;
        assert!((0.4..=0.6).contains(&initial_health));

        // Add some peers to improve health
        for i in 0..5 {
            monitor.record_peer_connected(format!("peer{}", i)).await;
        }

        let improved_health = monitor.network_health_score().await;
        assert!(improved_health > initial_health);
    }
}
