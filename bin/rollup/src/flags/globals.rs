//! Global arguments for the rollup CLI.

use clap::Parser;
use kona_cli::{log::LogArgs, node_config::NodeMode};
use url::Url;

/// Rollup binary that runs kona-node as an ExEx in op-reth.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "rollup",
    about = "Unified rollup binary with kona-node ExEx integration",
    version
)]
pub struct GlobalArgs {
    /// Logging arguments.
    #[command(flatten)]
    pub log_args: LogArgs,

    /// Kona node operation mode.
    #[arg(
        long = "kona.mode",
        default_value = "validator",
        env = "KONA_MODE",
        help = "Operation mode for kona-node (validator or sequencer)"
    )]
    pub mode: NodeMode,

    /// L1 Ethereum RPC URL.
    #[arg(
        long = "l1.eth",
        env = "L1_ETH_RPC",
        help = "URL of the L1 Ethereum execution client RPC API"
    )]
    pub l1_eth_rpc: Url,

    /// L1 beacon API URL.
    #[arg(
        long = "l1.beacon",
        env = "L1_BEACON",
        help = "URL of the L1 beacon API for blob fetching"
    )]
    pub l1_beacon: Url,

    /// L2 chain ID.
    #[arg(
        long = "chain",
        short = 'c',
        default_value = "10",
        env = "L2_CHAIN_ID",
        help = "L2 chain ID (e.g., 10 for Optimism, 8453 for Base)"
    )]
    pub l2_chain_id: u64,

    /// Custom rollup config file.
    #[arg(
        long = "kona.config-file",
        env = "KONA_CONFIG_FILE",
        help = "Path to custom rollup configuration file (overrides registry)"
    )]
    pub config_file: Option<std::path::PathBuf>,

    /// Whether to trust the L1 RPC.
    #[arg(
        long = "l1.trust-rpc",
        default_value = "true",
        env = "L1_TRUST_RPC",
        help = "Trust L1 RPC responses without verification"
    )]
    pub l1_trust_rpc: bool,

    /// P2P configuration.
    #[command(flatten)]
    pub p2p: super::P2PArgs,

    /// RPC configuration.
    #[command(flatten)]
    pub rpc: super::RpcArgs,

    /// Sequencer configuration.
    #[command(flatten)]
    pub sequencer: super::SequencerArgs,
}

impl Default for GlobalArgs {
    fn default() -> Self {
        Self {
            log_args: LogArgs::default(),
            mode: NodeMode::Validator,
            l1_eth_rpc: "http://localhost:8545".parse().unwrap(),
            l1_beacon: "http://localhost:5052".parse().unwrap(),
            l2_chain_id: 10,
            config_file: None,
            l1_trust_rpc: true,
            p2p: Default::default(),
            rpc: Default::default(),
            sequencer: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_global_args() {
        let args = GlobalArgs::default();
        assert_eq!(args.mode, NodeMode::Validator);
        assert_eq!(args.l2_chain_id, 10);
        assert!(args.l1_trust_rpc);
    }

    #[test]
    fn test_parse_mode() {
        let args = vec![
            "rollup",
            "--kona.mode",
            "sequencer",
            "--l1.eth",
            "http://localhost:8545",
            "--l1.beacon",
            "http://localhost:5052",
        ];
        let parsed = GlobalArgs::try_parse_from(args).unwrap();
        assert_eq!(parsed.mode, NodeMode::Sequencer);
    }

    #[test]
    fn test_parse_chain_id() {
        let args = vec![
            "rollup",
            "--chain",
            "8453",
            "--l1.eth",
            "http://localhost:8545",
            "--l1.beacon",
            "http://localhost:5052",
        ];
        let parsed = GlobalArgs::try_parse_from(args).unwrap();
        assert_eq!(parsed.l2_chain_id, 8453);
    }

    #[test]
    fn test_parse_required_urls() {
        let args = vec![
            "rollup",
            "--l1.eth",
            "http://eth.example.com",
            "--l1.beacon",
            "http://beacon.example.com",
        ];
        let parsed = GlobalArgs::try_parse_from(args).unwrap();
        assert_eq!(parsed.l1_eth_rpc.as_str(), "http://eth.example.com/");
        assert_eq!(parsed.l1_beacon.as_str(), "http://beacon.example.com/");
    }

    #[test]
    fn test_missing_required_args() {
        let args = vec!["rollup"];
        let result = GlobalArgs::try_parse_from(args);
        assert!(result.is_err());
    }
}
