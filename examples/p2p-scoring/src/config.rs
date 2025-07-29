//! Scoring configuration module
//!
//! This module provides comprehensive configuration for peer and topic scoring
//! in kona-p2p networks, allowing fine-tuned control over mesh behavior and
//! peer reputation management.

use anyhow::Result;
use kona_peers::{PeerMonitoring, PeerScoreLevel};
use libp2p::gossipsub::{Config, ConfigBuilder};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::profiles::ScoringProfile;

/// Comprehensive scoring configuration for kona P2P networks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    /// The scoring profile being used
    pub profile: ScoringProfile,
    /// Whether topic scoring is enabled
    pub topic_scoring: bool,
    /// Peer ban threshold
    pub ban_threshold: f64,
    /// Duration for which peers remain banned
    pub ban_duration: Duration,
    /// Block time for the network (used in scoring calculations)
    pub block_time: u64,
    /// Gossipsub mesh parameters
    pub mesh_config: MeshConfig,
    /// Heartbeat and timing configuration
    pub timing_config: TimingConfig,
}

/// Mesh network configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    /// Target number of peers in mesh (D)
    pub mesh_n: usize,
    /// Lower bound for mesh peers (D_lo)
    pub mesh_n_low: usize,
    /// Upper bound for mesh peers (D_hi)  
    pub mesh_n_high: usize,
    /// Number of peers for lazy gossip (D_lazy)
    pub gossip_lazy: usize,
    /// Whether to flood publish messages
    pub flood_publish: bool,
}

/// Network timing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    /// Gossip heartbeat interval
    pub heartbeat_interval: Duration,
    /// Fanout TTL
    pub fanout_ttl: Duration,
    /// History length for message gossip
    pub history_length: usize,
    /// History gossip factor
    pub history_gossip: usize,
}

impl ScoringConfig {
    /// Create a new scoring configuration.
    pub fn new(
        profile: ScoringProfile,
        topic_scoring: bool,
        ban_threshold: Option<f64>,
        ban_duration: Option<Duration>,
        block_time: u64,
    ) -> Result<Self> {
        let (default_ban_threshold, default_ban_duration) = match profile {
            ScoringProfile::Off => (0.0, Duration::from_secs(0)),
            ScoringProfile::Conservative => (-20.0, Duration::from_secs(300)), // 5 minutes
            ScoringProfile::Moderate => (-40.0, Duration::from_secs(900)),     // 15 minutes
            ScoringProfile::Aggressive => (-60.0, Duration::from_secs(1800)),  // 30 minutes
        };

        let mesh_config = match profile {
            ScoringProfile::Off => MeshConfig {
                mesh_n: 6, // Smaller mesh for testing
                mesh_n_low: 4,
                mesh_n_high: 8,
                gossip_lazy: 4,
                flood_publish: true, // Flood publish when no scoring
            },
            ScoringProfile::Conservative => MeshConfig {
                mesh_n: 8, // Standard mesh size
                mesh_n_low: 6,
                mesh_n_high: 12,
                gossip_lazy: 6,
                flood_publish: false,
            },
            ScoringProfile::Moderate => MeshConfig {
                mesh_n: 10, // Larger mesh for reliability
                mesh_n_low: 8,
                mesh_n_high: 15,
                gossip_lazy: 8,
                flood_publish: false,
            },
            ScoringProfile::Aggressive => MeshConfig {
                mesh_n: 12, // Large mesh for high performance
                mesh_n_low: 10,
                mesh_n_high: 18,
                gossip_lazy: 10,
                flood_publish: false,
            },
        };

        let timing_config = TimingConfig {
            heartbeat_interval: Duration::from_millis(500),
            fanout_ttl: Duration::from_secs(60),
            history_length: 12,
            history_gossip: 3,
        };

        Ok(Self {
            profile,
            topic_scoring,
            ban_threshold: ban_threshold.unwrap_or(default_ban_threshold),
            ban_duration: ban_duration.unwrap_or(default_ban_duration),
            block_time,
            mesh_config,
            timing_config,
        })
    }

    /// Get the peer score level for this configuration.
    pub fn peer_score_level(&self) -> PeerScoreLevel {
        match self.profile {
            ScoringProfile::Off => PeerScoreLevel::Off,
            ScoringProfile::Conservative |
            ScoringProfile::Moderate |
            ScoringProfile::Aggressive => PeerScoreLevel::Light,
        }
    }

    /// Get peer monitoring configuration if scoring is enabled.
    pub fn peer_monitoring(&self) -> Option<PeerMonitoring> {
        match self.profile {
            ScoringProfile::Off => None,
            _ => Some(PeerMonitoring {
                ban_threshold: self.ban_threshold,
                ban_duration: self.ban_duration,
            }),
        }
    }

    /// Create a gossipsub configuration based on this scoring config.
    pub fn gossip_config(&self) -> Config {
        let mut builder = ConfigBuilder::default();

        builder
            .mesh_n(self.mesh_config.mesh_n)
            .mesh_n_low(self.mesh_config.mesh_n_low)
            .mesh_n_high(self.mesh_config.mesh_n_high)
            .gossip_lazy(self.mesh_config.gossip_lazy)
            .heartbeat_interval(self.timing_config.heartbeat_interval)
            .fanout_ttl(self.timing_config.fanout_ttl)
            .history_length(self.timing_config.history_length)
            .history_gossip(self.timing_config.history_gossip)
            .flood_publish(self.mesh_config.flood_publish)
            .support_floodsub()
            .max_transmit_size(10 * (1 << 20)) // 10MB
            .duplicate_cache_time(Duration::from_secs(120))
            .validation_mode(libp2p::gossipsub::ValidationMode::None)
            .validate_messages()
            .message_id_fn(|msg| {
                use libp2p::gossipsub::MessageId;
                use openssl::sha::sha256;
                use snap::raw::Decoder;

                let mut decoder = Decoder::new();
                let id = decoder.decompress_vec(&msg.data).map_or_else(
                    |_| {
                        let domain_invalid_snappy: Vec<u8> = vec![0x0, 0x0, 0x0, 0x0];
                        sha256(
                            [domain_invalid_snappy.as_slice(), msg.data.as_slice()]
                                .concat()
                                .as_slice(),
                        )[..20]
                            .to_vec()
                    },
                    |data| {
                        let domain_valid_snappy: Vec<u8> = vec![0x1, 0x0, 0x0, 0x0];
                        sha256(
                            [domain_valid_snappy.as_slice(), data.as_slice()].concat().as_slice(),
                        )[..20]
                            .to_vec()
                    },
                );
                MessageId(id)
            });

        builder.build().expect("Gossipsub configuration should be valid")
    }

    /// Print detailed configuration information.
    pub fn print_detailed_config(&self) {
        println!("=== Kona P2P Scoring Configuration ===\n");

        println!("Profile: {:?}", self.profile);
        println!("Topic Scoring: {}", self.topic_scoring);
        println!("Block Time: {} seconds", self.block_time);

        if self.profile != ScoringProfile::Off {
            println!("\n--- Peer Scoring ---");
            println!("Ban Threshold: {}", self.ban_threshold);
            println!("Ban Duration: {:?}", self.ban_duration);
        }

        println!("\n--- Mesh Configuration ---");
        println!("Mesh Size (D): {}", self.mesh_config.mesh_n);
        println!("Mesh Low (D_lo): {}", self.mesh_config.mesh_n_low);
        println!("Mesh High (D_hi): {}", self.mesh_config.mesh_n_high);
        println!("Gossip Lazy (D_lazy): {}", self.mesh_config.gossip_lazy);
        println!("Flood Publish: {}", self.mesh_config.flood_publish);

        println!("\n--- Timing Configuration ---");
        println!("Heartbeat Interval: {:?}", self.timing_config.heartbeat_interval);
        println!("Fanout TTL: {:?}", self.timing_config.fanout_ttl);
        println!("History Length: {}", self.timing_config.history_length);
        println!("History Gossip: {}", self.timing_config.history_gossip);

        if self.topic_scoring {
            println!("\n--- Topic Scoring Parameters ---");
            if let Some(params) = self.peer_score_level().to_params(vec![], true, self.block_time) {
                println!("Topic Score Cap: {}", params.topic_score_cap);
                println!("App Specific Weight: {}", params.app_specific_weight);
                println!("IP Colocation Factor Weight: {}", params.ip_colocation_factor_weight);
                println!(
                    "IP Colocation Factor Threshold: {}",
                    params.ip_colocation_factor_threshold
                );
                println!("Behaviour Penalty Weight: {}", params.behaviour_penalty_weight);
                println!("Behaviour Penalty Threshold: {}", params.behaviour_penalty_threshold);
                println!("Behaviour Penalty Decay: {}", params.behaviour_penalty_decay);
                println!("Decay Interval: {:?}", params.decay_interval);
                println!("Decay to Zero: {}", params.decay_to_zero);
                println!("Retain Score: {:?}", params.retain_score);
            }
        }

        println!("\n=== End Configuration ===");
    }

    /// Log the current configuration using tracing.
    pub fn log_configuration(&self) {
        tracing::info!(
            target: "scoring",
            "Scoring configuration: profile={:?}, topic_scoring={}, ban_threshold={}, ban_duration={:?}",
            self.profile,
            self.topic_scoring,
            self.ban_threshold,
            self.ban_duration
        );

        tracing::info!(
            target: "scoring",
            "Mesh configuration: D={}, D_lo={}, D_hi={}, D_lazy={}, flood_publish={}",
            self.mesh_config.mesh_n,
            self.mesh_config.mesh_n_low,
            self.mesh_config.mesh_n_high,
            self.mesh_config.gossip_lazy,
            self.mesh_config.flood_publish
        );

        tracing::debug!(
            target: "scoring",
            "Timing configuration: heartbeat={:?}, fanout_ttl={:?}, history_length={}, history_gossip={}",
            self.timing_config.heartbeat_interval,
            self.timing_config.fanout_ttl,
            self.timing_config.history_length,
            self.timing_config.history_gossip
        );
    }

    /// Export configuration as JSON.
    #[allow(dead_code)]
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(Into::into)
    }

    /// Import configuration from JSON.
    #[allow(dead_code)]
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(Into::into)
    }
}
