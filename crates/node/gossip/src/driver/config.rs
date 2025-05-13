//! Configuration.

use alloy_primitives::Address;
use kona_genesis::RollupConfig;
use kona_peers::{PeerMonitoring, PeerScoreLevel};
use libp2p::identity::Keypair;
use tokio::time::Duration;

/// Configuration for the gossip layer of kona's P2P stack.
#[derive(Debug, Clone)]
pub struct Config {
    /// The gossip address.
    pub gossip_address: libp2p::Multiaddr,
    /// The unsafe block signer.
    pub unsafe_block_signer: Address,
    /// The keypair.
    pub keypair: Keypair,
    /// The gossip config.
    pub gossip_config: libp2p::gossipsub::Config,
    /// The peer score level.
    pub scoring: PeerScoreLevel,
    /// Peer score monitoring config.
    pub monitor_peers: Option<PeerMonitoring>,
    /// The L2 Block Time.
    pub block_time: u64,
    /// The optional number of times to redial a peer.
    pub redial: Option<u64>,
    /// The [`RollupConfig`].
    pub rollup_config: RollupConfig,
}
