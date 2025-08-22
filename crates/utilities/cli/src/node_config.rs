//! Node CLI Configuration for ExEx integration.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

#[cfg(feature = "node-config")]
use alloy_chains::Chain;
#[cfg(feature = "node-config")]
use alloy_primitives::Address;

/// Node operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeMode {
    /// Validator mode - validates L2 blocks against derivation pipeline
    Validator,
    /// Sequencer mode - produces new L2 blocks
    Sequencer,
}

impl Default for NodeMode {
    fn default() -> Self {
        Self::Validator
    }
}

impl std::fmt::Display for NodeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validator => write!(f, "validator"),
            Self::Sequencer => write!(f, "sequencer"),
        }
    }
}

impl std::str::FromStr for NodeMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "validator" => Ok(Self::Validator),
            "sequencer" => Ok(Self::Sequencer),
            _ => anyhow::bail!("Invalid node mode: {}", s),
        }
    }
}

/// Lightweight P2P configuration for ExEx integration
///
/// This struct contains the minimal P2P configuration needed for ExEx integration,
/// extracting core fields from the full P2PArgs structure in the kona-node binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct P2PConfig {
    /// Whether to disable node discovery
    pub no_discovery: bool,
    /// Private key file path for peer identity
    pub priv_path: Option<PathBuf>,
    /// Listen IP address for P2P networking
    pub listen_ip: std::net::IpAddr,
    /// TCP port for libp2p
    pub listen_tcp_port: u16,
    /// UDP port for discovery
    pub listen_udp_port: u16,
    /// Low-tide peer count
    pub peers_lo: u32,
    /// High-tide peer count  
    pub peers_hi: u32,
    /// Peer score level for banning
    pub scoring: String,
    /// Enable peer banning
    pub ban_enabled: bool,
    /// Unsafe block signer address override
    pub unsafe_block_signer: Option<Address>,
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            no_discovery: false,
            priv_path: None,
            listen_ip: "0.0.0.0".parse().unwrap(),
            listen_tcp_port: 9222,
            listen_udp_port: 9223,
            peers_lo: 20,
            peers_hi: 30,
            scoring: "light".to_string(),
            ban_enabled: false,
            unsafe_block_signer: None,
        }
    }
}

/// Lightweight RPC configuration for ExEx integration
///
/// This struct contains the minimal RPC configuration needed for ExEx integration,
/// extracting core fields from the full RpcArgs structure in the kona-node binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Whether RPC is disabled
    pub disabled: bool,
    /// RPC listening address
    pub listen_addr: std::net::IpAddr,
    /// RPC listening port
    pub listen_port: u16,
    /// Enable admin API
    pub enable_admin: bool,
    /// Admin persistence file path
    pub admin_persistence: Option<PathBuf>,
    /// Enable WebSocket RPC
    pub ws_enabled: bool,
    /// Enable development RPC endpoints
    pub dev_enabled: bool,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            listen_addr: "0.0.0.0".parse().unwrap(),
            listen_port: 9545,
            enable_admin: false,
            admin_persistence: None,
            ws_enabled: false,
            dev_enabled: false,
        }
    }
}

/// Lightweight Sequencer configuration for ExEx integration
///
/// This struct contains the minimal sequencer configuration needed for ExEx integration,
/// extracting core fields from the full SequencerArgs structure in the kona-node binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencerConfig {
    /// Start sequencer in stopped state
    pub stopped: bool,
    /// Maximum safe lag blocks
    pub max_safe_lag: u64,
    /// L1 confirmations required
    pub l1_confs: u64,
    /// Enable recovery mode
    pub recover: bool,
    /// Conductor RPC URL
    pub conductor_rpc: Option<Url>,
}

impl Default for SequencerConfig {
    fn default() -> Self {
        Self { stopped: false, max_safe_lag: 0, l1_confs: 4, recover: false, conductor_rpc: None }
    }
}

/// Global configuration for ExEx integration
///
/// Contains global settings like chain ID and fork overrides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// L2 chain ID
    #[cfg(feature = "node-config")]
    pub l2_chain_id: Chain,
    #[cfg(not(feature = "node-config"))]
    pub l2_chain_id: u64,

    /// Fork override configuration
    pub fork_overrides: ForkOverrides,
}

/// Fork override configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkOverrides {
    /// Canyon fork timestamp override
    pub canyon_override: Option<u64>,
    /// Delta fork timestamp override
    pub delta_override: Option<u64>,
    /// Ecotone fork timestamp override
    pub ecotone_override: Option<u64>,
    /// Fjord fork timestamp override
    pub fjord_override: Option<u64>,
    /// Granite fork timestamp override
    pub granite_override: Option<u64>,
    /// Holocene fork timestamp override
    pub holocene_override: Option<u64>,
    /// Pectra blob schedule timestamp override
    pub pectra_blob_schedule_override: Option<u64>,
    /// Isthmus fork timestamp override
    pub isthmus_override: Option<u64>,
    /// Jovian fork timestamp override
    pub jovian_override: Option<u64>,
    /// Interop fork timestamp override
    pub interop_override: Option<u64>,
}

impl Default for ForkOverrides {
    fn default() -> Self {
        Self {
            canyon_override: None,
            delta_override: None,
            ecotone_override: None,
            fjord_override: None,
            granite_override: None,
            holocene_override: None,
            pectra_blob_schedule_override: None,
            isthmus_override: None,
            jovian_override: None,
            interop_override: None,
        }
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            #[cfg(feature = "node-config")]
            l2_chain_id: Chain::from_id(10),
            #[cfg(not(feature = "node-config"))]
            l2_chain_id: 10,
            fork_overrides: Default::default(),
        }
    }
}

/// Main node CLI configuration structure for ExEx integration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCliConfig {
    /// Node operation mode
    pub mode: NodeMode,
    /// L1 Ethereum RPC URL
    pub l1_eth_rpc: Url,
    /// Trust L1 RPC (disable verification)
    pub l1_trust_rpc: bool,
    /// L1 Beacon API URL
    pub l1_beacon: Url,
    /// Trust L2 RPC (disable verification)
    pub l2_trust_rpc: bool,
    /// Custom L2 configuration file path
    pub l2_config_file: Option<PathBuf>,
    /// P2P networking configuration
    pub p2p: P2PConfig,
    /// RPC server configuration
    pub rpc: RpcConfig,
    /// Sequencer configuration
    pub sequencer: SequencerConfig,
    /// Global configuration
    pub global: GlobalConfig,
}

impl Default for NodeCliConfig {
    fn default() -> Self {
        Self {
            mode: Default::default(),
            l1_eth_rpc: "http://localhost:8545".parse().unwrap(),
            l1_trust_rpc: false,
            l1_beacon: "http://localhost:5052".parse().unwrap(),
            l2_trust_rpc: false,
            l2_config_file: None,
            p2p: Default::default(),
            rpc: Default::default(),
            sequencer: Default::default(),
            global: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_mode_from_str() {
        assert_eq!("validator".parse::<NodeMode>().unwrap(), NodeMode::Validator);
        assert_eq!("sequencer".parse::<NodeMode>().unwrap(), NodeMode::Sequencer);
        assert!("invalid".parse::<NodeMode>().is_err());
    }

    #[test]
    fn test_node_mode_display() {
        assert_eq!(NodeMode::Validator.to_string(), "validator");
        assert_eq!(NodeMode::Sequencer.to_string(), "sequencer");
    }

    #[test]
    fn test_default_configs() {
        let config = NodeCliConfig::default();
        assert_eq!(config.mode, NodeMode::Validator);
        assert!(!config.l1_trust_rpc);
        assert!(!config.l2_trust_rpc);
        assert_eq!(config.p2p.listen_tcp_port, 9222);
        assert_eq!(config.rpc.listen_port, 9545);
        assert_eq!(config.sequencer.l1_confs, 4);
    }

    #[test]
    fn test_config_modification() {
        let mut config = NodeCliConfig::default();
        config.mode = NodeMode::Sequencer;
        config.l1_trust_rpc = true;

        assert_eq!(config.mode, NodeMode::Sequencer);
        assert!(config.l1_trust_rpc);
    }

    #[test]
    fn test_serialization() {
        let config = NodeCliConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: NodeCliConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }
}
