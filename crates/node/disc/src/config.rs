//! Configuration.

use discv5::Enr;
use kona_disc::LocalNode;
use kona_genesis::RollupConfig;
use std::path::PathBuf;
use tokio::time::Duration;

/// Configuration for the discovery part of kona's P2P stack.
#[derive(Debug, Clone)]
pub struct Config {
    /// Discovery Config.
    pub discovery_config: discv5::Config,
    /// The local node's advertised address to external peers.
    /// Note: This may be different from the node's discovery listen address.
    pub discovery_address: LocalNode,
    /// The interval to find peers.
    pub discovery_interval: Duration,
    /// The interval to remove peers from the discovery service.
    pub discovery_randomize: Option<Duration>,
    /// An optional path to the bootstore.
    pub bootstore: Option<PathBuf>,
    /// An optional list of bootnode ENRs to start the node with.
    pub bootnodes: Vec<Enr>,
    /// An l2 chain id.
    pub l2_chain_id: u64,
}
