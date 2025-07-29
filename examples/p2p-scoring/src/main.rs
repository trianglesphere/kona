//! Comprehensive example demonstrating kona-p2p scoring configuration
//!
//! This example showcases various peer and topic scoring configurations that can be used
//! in real-world kona-based applications. It demonstrates how to:
//!
//! - Configure different peer scoring levels (Off, Light)
//! - Set up topic scoring parameters for optimal mesh performance
//! - Implement peer monitoring with ban thresholds
//! - Handle different network scenarios (conservative, moderate, aggressive)
//!
//! ## Usage
//!
//! ```sh
//! # Run with conservative scoring (default)
//! cargo run --release -p example-p2p-scoring
//!
//! # Run with different scoring profiles
//! cargo run --release -p example-p2p-scoring -- --scoring-profile moderate
//! cargo run --release -p example-p2p-scoring -- --scoring-profile aggressive
//!
//! # Disable scoring entirely
//! cargo run --release -p example-p2p-scoring -- --scoring-profile off
//!
//! # Enable topic scoring
//! cargo run --release -p example-p2p-scoring -- --topic-scoring
//!
//! # Custom configuration
//! cargo run --release -p example-p2p-scoring -- --ban-threshold -50.0 --ban-duration 300
//! ```
//!
//! ## Scoring Profiles
//!
//! - **Off**: No peer scoring applied - useful for testing or permissioned networks
//! - **Conservative**: Light scoring with lenient thresholds - good for new networks
//! - **Moderate**: Balanced scoring suitable for most production environments
//! - **Aggressive**: Strict scoring for high-security or high-performance requirements

#![warn(unused_crate_dependencies)]

use anyhow::Result;
use clap::Parser;
use discv5::enr::CombinedKey;
use kona_cli::{LogConfig, log::LogArgs};
use kona_node_service::{NetworkActor, NetworkConfig, NetworkContext, NodeActor};
use kona_p2p::LocalNode;
use kona_registry::ROLLUP_CONFIGS;
use libp2p::{Multiaddr, identity::Keypair};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

mod config;
mod monitoring;
mod profiles;

use config::ScoringConfig;
use monitoring::NetworkMonitor;
use profiles::ScoringProfile;

/// The scoring command demonstrating various P2P scoring configurations.
#[derive(Parser, Debug, Clone)]
#[command(
    about = "Demonstrates kona-p2p scoring configuration patterns",
    long_about = "A comprehensive example showing how to configure peer and topic scoring \
                  for different network scenarios in kona-based applications."
)]
pub struct ScoringCommand {
    #[command(flatten)]
    pub logging: LogArgs,

    /// The L2 chain ID to use.
    #[arg(long, short = 'c', default_value = "10", help = "The L2 chain ID to use")]
    pub l2_chain_id: u64,

    /// Port to listen for gossip on.
    #[arg(long, short = 'g', default_value = "9099", help = "Port to listen for gossip on")]
    pub gossip_port: u16,

    /// Port to listen for discovery on.
    #[arg(long, short = 'd', default_value = "9098", help = "Port to listen for discovery on")]
    pub disc_port: u16,

    /// Interval to send discovery packets.
    #[arg(
        long,
        short = 'i',
        default_value = "5",
        help = "Interval to send discovery packets in seconds"
    )]
    pub discovery_interval: u64,

    /// Scoring profile to use.
    #[arg(
        long,
        short = 's',
        default_value = "conservative",
        help = "Scoring profile: off, conservative, moderate, aggressive"
    )]
    pub scoring_profile: ScoringProfile,

    /// Enable topic scoring.
    #[arg(long, help = "Enable topic scoring for enhanced mesh performance")]
    pub topic_scoring: bool,

    /// Peer ban threshold.
    #[arg(long, help = "Threshold below which peers are banned (negative values)")]
    pub ban_threshold: Option<f64>,

    /// Peer ban duration in seconds.
    #[arg(long, help = "Duration for which banned peers remain banned")]
    pub ban_duration: Option<u64>,

    /// Enable network monitoring.
    #[arg(long, default_value = "true", help = "Enable network monitoring and metrics collection")]
    pub monitoring: bool,

    /// Output detailed scoring configuration.
    #[arg(long, help = "Print detailed scoring configuration and exit")]
    pub show_config: bool,
}

impl ScoringCommand {
    /// Run the scoring example.
    pub async fn run(self) -> Result<()> {
        // Initialize logging
        LogConfig::new(self.logging).init_tracing_subscriber(None::<EnvFilter>)?;

        // Load rollup configuration
        let rollup_config = ROLLUP_CONFIGS.get(&self.l2_chain_id).ok_or_else(|| {
            anyhow::anyhow!("No rollup config found for chain ID {}", self.l2_chain_id)
        })?;

        let signer = rollup_config
            .genesis
            .system_config
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!("No system config found for chain ID {}", self.l2_chain_id)
            })?
            .batcher_address;

        // Create scoring configuration
        let scoring_config = ScoringConfig::new(
            self.scoring_profile,
            self.topic_scoring,
            self.ban_threshold,
            self.ban_duration.map(Duration::from_secs),
            rollup_config.block_time,
        )?;

        // Show configuration if requested
        if self.show_config {
            scoring_config.print_detailed_config();
            return Ok(());
        }

        tracing::info!(
            target: "scoring",
            "Starting P2P scoring example with profile: {:?}",
            self.scoring_profile
        );

        // Set up network addresses
        let gossip_socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), self.gossip_port);
        let mut gossip_addr = Multiaddr::from(gossip_socket.ip());
        gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(gossip_socket.port()));

        // Generate keypair and discovery node
        let CombinedKey::Secp256k1(secret_key) = CombinedKey::generate_secp256k1() else {
            unreachable!("Generated key should be secp256k1")
        };

        let disc_ip = Ipv4Addr::UNSPECIFIED;
        let disc_addr =
            LocalNode::new(secret_key, IpAddr::V4(disc_ip), self.disc_port, self.disc_port);

        // Configure peer monitoring if enabled
        let peer_monitoring = scoring_config.peer_monitoring();

        // Create network configuration
        let network_config = NetworkConfig {
            discovery_address: disc_addr,
            gossip_address: gossip_addr,
            unsafe_block_signer: signer,
            discovery_config: discv5::ConfigBuilder::new(discv5::ListenConfig::Ipv4 {
                ip: disc_ip,
                port: self.disc_port,
            })
            .build(),
            discovery_interval: Duration::from_secs(self.discovery_interval),
            discovery_randomize: None,
            keypair: Keypair::generate_secp256k1(),
            gossip_config: scoring_config.gossip_config(),
            scoring: scoring_config.peer_score_level(),
            topic_scoring: self.topic_scoring,
            monitor_peers: peer_monitoring,
            bootstore: None,
            gater_config: Default::default(),
            bootnodes: Default::default(),
            rollup_config: rollup_config.clone(),
            local_signer: None,
        }
        .into();

        // Create and start network actor
        let (_, network) = NetworkActor::new(network_config);

        let (unsafe_blocks_tx, mut unsafe_blocks_rx) = tokio::sync::mpsc::channel(1024);
        let cancellation_token = CancellationToken::new();

        // Start network monitoring if enabled
        let mut monitor =
            if self.monitoring { Some(NetworkMonitor::new(scoring_config.clone())) } else { None };

        // Start the network
        network
            .start(NetworkContext {
                blocks: unsafe_blocks_tx,
                cancellation: cancellation_token.clone(),
            })
            .await?;

        tracing::info!(
            target: "scoring",
            "Network started successfully. Peer scoring profile: {:?}, Topic scoring: {}",
            self.scoring_profile,
            self.topic_scoring
        );

        tracing::info!(
            target: "scoring",
            "Listening for gossip on: {}, discovery on: {}",
            gossip_socket,
            SocketAddr::new(IpAddr::V4(disc_ip), self.disc_port)
        );

        // Log scoring configuration details
        scoring_config.log_configuration();

        // Main event loop
        let mut block_count = 0u64;
        let mut last_stats = std::time::Instant::now();

        loop {
            tokio::select! {
                // Handle incoming blocks
                block = unsafe_blocks_rx.recv() => {
                    match block {
                        Some(block) => {
                            block_count += 1;
                            tracing::info!(
                                target: "scoring",
                                "Received unsafe block #{}: {:?}",
                                block_count,
                                block
                            );

                            // Update monitoring stats
                            if let Some(ref mut monitor) = monitor {
                                monitor.record_block_received().await;
                            }
                        }
                        None => {
                            tracing::warn!(target: "scoring", "Block channel closed");
                            break;
                        }
                    }
                }

                // Periodic stats reporting
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    let elapsed = last_stats.elapsed();
                    last_stats = std::time::Instant::now();

                    tracing::info!(
                        target: "scoring",
                        "Stats: {} blocks received in the last {:?}",
                        block_count,
                        elapsed
                    );

                    if let Some(ref mut monitor) = monitor {
                        monitor.log_network_stats().await;
                    }
                }

                // Handle cancellation
                _ = cancellation_token.cancelled() => {
                    tracing::info!(target: "scoring", "Received cancellation signal, shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(err) = ScoringCommand::parse().run().await {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
    Ok(())
}
