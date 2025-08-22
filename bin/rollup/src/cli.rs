//! CLI for the unified rollup binary.

use clap::{Args, Parser, Subcommand};
use kona_cli::{NodeCliConfig, NodeMode, P2PConfig, RpcConfig, SequencerConfig};
use std::path::PathBuf;
use url::Url;

/// Rollup CLI with kona node arguments.
#[derive(Debug, Parser)]
#[command(name = "rollup", about = "Unified rollup binary with Kona Node ExEx")]
pub struct RollupCli {
    /// Rollup subcommand
    #[command(subcommand)]
    pub command: RollupCommand,
    /// Kona node arguments
    #[command(flatten)]
    pub kona: KonaNodeArgs,
}

/// Rollup commands.
#[derive(Debug, Subcommand)]
pub enum RollupCommand {
    /// Start the rollup node
    Node(NodeCommand),
}

/// Node command arguments.
#[derive(Debug, Args)]
pub struct NodeCommand {
    /// L1 Ethereum RPC URL
    #[arg(long = "l1.eth")]
    pub l1_eth_rpc: Option<Url>,

    /// L1 Beacon API URL
    #[arg(long = "l1.beacon")]
    pub l1_beacon_rpc: Option<Url>,

    /// Chain ID for the L2 network
    #[arg(long = "chain")]
    pub chain_id: Option<u64>,

    /// Data directory for the node
    #[arg(long = "datadir")]
    pub data_dir: Option<PathBuf>,
}

/// Kona node arguments with kona.* prefixes.
#[derive(Debug, Args)]
#[command(next_help_heading = "Kona Node Options")]
pub struct KonaNodeArgs {
    /// Kona node operation mode
    #[arg(long = "kona.mode", value_enum, default_value = "validator")]
    pub mode: CliNodeMode,

    /// Trust L1 RPC (disable verification)
    #[arg(long = "kona.l1-trust-rpc")]
    pub l1_trust_rpc: bool,

    /// Trust L2 RPC (disable verification)
    #[arg(long = "kona.l2-trust-rpc")]
    pub l2_trust_rpc: bool,

    /// Custom rollup config file (overrides registry)
    #[arg(long = "kona.config-file")]
    pub config_file: Option<PathBuf>,

    /// P2P configuration
    #[command(flatten)]
    pub p2p: KonaP2PArgs,

    /// RPC configuration for kona (prefixed to avoid conflicts)
    #[command(flatten)]
    pub rpc: KonaRpcArgs,

    /// Sequencer configuration
    #[command(flatten)]
    pub sequencer: KonaSequencerArgs,
}

/// Node operation mode.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliNodeMode {
    Validator,
    Sequencer,
}

impl From<CliNodeMode> for NodeMode {
    fn from(mode: CliNodeMode) -> Self {
        match mode {
            CliNodeMode::Validator => NodeMode::Validator,
            CliNodeMode::Sequencer => NodeMode::Sequencer,
        }
    }
}

/// P2P configuration with kona.* prefixes.
#[derive(Debug, Clone, Args)]
#[command(next_help_heading = "Kona P2P Options")]
pub struct KonaP2PArgs {
    /// Disable node discovery
    #[arg(long = "kona.p2p.no-discovery")]
    pub no_discovery: bool,

    /// Private key file path for peer identity
    #[arg(long = "kona.p2p.priv-path")]
    pub priv_path: Option<PathBuf>,

    /// Listen IP address for P2P networking
    #[arg(long = "kona.p2p.listen-ip", default_value = "0.0.0.0")]
    pub listen_ip: std::net::IpAddr,

    /// TCP port for libp2p
    #[arg(long = "kona.p2p.listen-tcp-port", default_value = "9222")]
    pub listen_tcp_port: u16,

    /// UDP port for discovery
    #[arg(long = "kona.p2p.listen-udp-port", default_value = "9223")]
    pub listen_udp_port: u16,

    /// Low-tide peer count
    #[arg(long = "kona.p2p.peers-lo", default_value = "20")]
    pub peers_lo: u32,

    /// High-tide peer count
    #[arg(long = "kona.p2p.peers-hi", default_value = "30")]
    pub peers_hi: u32,

    /// Peer score level for banning
    #[arg(long = "kona.p2p.scoring", default_value = "light")]
    pub scoring: String,

    /// Enable peer banning
    #[arg(long = "kona.p2p.ban-enabled")]
    pub ban_enabled: bool,
}

impl From<KonaP2PArgs> for P2PConfig {
    fn from(args: KonaP2PArgs) -> Self {
        Self {
            no_discovery: args.no_discovery,
            priv_path: args.priv_path,
            listen_ip: args.listen_ip,
            listen_tcp_port: args.listen_tcp_port,
            listen_udp_port: args.listen_udp_port,
            peers_lo: args.peers_lo,
            peers_hi: args.peers_hi,
            scoring: args.scoring,
            ban_enabled: args.ban_enabled,
            unsafe_block_signer: None, // Not exposed via CLI for security
        }
    }
}

/// RPC configuration with kona.* prefixes.
#[derive(Debug, Clone, Args)]
#[command(next_help_heading = "Kona RPC Options")]
pub struct KonaRpcArgs {
    /// Disable Kona RPC server
    #[arg(long = "kona.rpc.disabled")]
    pub disabled: bool,

    /// Kona RPC listening address
    #[arg(long = "kona.rpc.addr", default_value = "0.0.0.0")]
    pub listen_addr: std::net::IpAddr,

    /// Kona RPC listening port
    #[arg(long = "kona.rpc.port", default_value = "9546")]
    pub listen_port: u16,

    /// Enable admin API for Kona
    #[arg(long = "kona.rpc.enable-admin")]
    pub enable_admin: bool,

    /// Admin persistence file path
    #[arg(long = "kona.rpc.admin-persistence")]
    pub admin_persistence: Option<PathBuf>,

    /// Enable WebSocket RPC for Kona
    #[arg(long = "kona.rpc.ws-enabled")]
    pub ws_enabled: bool,

    /// Enable development RPC endpoints for Kona
    #[arg(long = "kona.rpc.dev-enabled")]
    pub dev_enabled: bool,
}

impl From<KonaRpcArgs> for RpcConfig {
    fn from(args: KonaRpcArgs) -> Self {
        Self {
            disabled: args.disabled,
            listen_addr: args.listen_addr,
            listen_port: args.listen_port,
            enable_admin: args.enable_admin,
            admin_persistence: args.admin_persistence,
            ws_enabled: args.ws_enabled,
            dev_enabled: args.dev_enabled,
        }
    }
}

/// Sequencer configuration with kona.* prefixes.
#[derive(Debug, Clone, Args)]
#[command(next_help_heading = "Kona Sequencer Options")]
pub struct KonaSequencerArgs {
    /// Start sequencer in stopped state
    #[arg(long = "kona.sequencer.stopped")]
    pub stopped: bool,

    /// Maximum safe lag blocks
    #[arg(long = "kona.sequencer.max-safe-lag", default_value = "0")]
    pub max_safe_lag: u64,

    /// L1 confirmations required
    #[arg(long = "kona.sequencer.l1-confs", default_value = "4")]
    pub l1_confs: u64,

    /// Enable recovery mode
    #[arg(long = "kona.sequencer.recover")]
    pub recover: bool,

    /// Conductor RPC URL
    #[arg(long = "kona.sequencer.conductor-rpc")]
    pub conductor_rpc: Option<Url>,
}

impl From<KonaSequencerArgs> for SequencerConfig {
    fn from(args: KonaSequencerArgs) -> Self {
        Self {
            stopped: args.stopped,
            max_safe_lag: args.max_safe_lag,
            l1_confs: args.l1_confs,
            recover: args.recover,
            conductor_rpc: args.conductor_rpc,
        }
    }
}

impl RollupCli {
    /// Creates NodeCliConfig from parsed arguments.
    pub fn create_kona_config(&self) -> anyhow::Result<NodeCliConfig> {
        let node_cmd = match &self.command {
            RollupCommand::Node(cmd) => cmd,
        };

        let mut config = NodeCliConfig {
            mode: self.kona.mode.into(),
            l1_trust_rpc: self.kona.l1_trust_rpc,
            l2_trust_rpc: self.kona.l2_trust_rpc,
            l2_config_file: self.kona.config_file.clone(),
            p2p: self.kona.p2p.clone().into(),
            rpc: self.kona.rpc.clone().into(),
            sequencer: self.kona.sequencer.clone().into(),
            ..Default::default()
        };

        if let Some(url) = &node_cmd.l1_eth_rpc {
            config.l1_eth_rpc = url.clone();
        }

        if let Some(url) = &node_cmd.l1_beacon_rpc {
            config.l1_beacon = url.clone();
        }

        if let Some(chain_id) = node_cmd.chain_id {
            config.global.l2_chain_id = chain_id.into();
        }

        Ok(config)
    }

    /// Validates CLI arguments.
    pub fn validate(&self) -> anyhow::Result<()> {
        let node_cmd = match &self.command {
            RollupCommand::Node(cmd) => cmd,
        };

        if node_cmd.l1_eth_rpc.is_none() {
            anyhow::bail!("L1 Ethereum RPC URL is required (--l1.eth)");
        }

        if node_cmd.l1_beacon_rpc.is_none() {
            anyhow::bail!("L1 Beacon RPC URL is required (--l1.beacon)");
        }

        if self.kona.p2p.listen_tcp_port == self.kona.p2p.listen_udp_port {
            anyhow::bail!("P2P TCP and UDP ports cannot be the same");
        }

        let conflicting_ports = [8545, 8546, 8551];
        if conflicting_ports.contains(&self.kona.rpc.listen_port) {
            anyhow::bail!("Kona RPC port {} conflicts with reth ports", self.kona.rpc.listen_port);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parsing() {
        let args = vec![
            "rollup",
            "--kona.mode",
            "validator",
            "--kona.rpc.port",
            "9546",
            "node",
            "--l1.eth",
            "http://localhost:8545",
            "--l1.beacon",
            "http://localhost:5052",
        ];

        let cli = RollupCli::try_parse_from(args).unwrap();

        match cli.command {
            RollupCommand::Node(node_cmd) => {
                assert_eq!(node_cmd.l1_eth_rpc.unwrap().to_string(), "http://localhost:8545/");
                assert_eq!(node_cmd.l1_beacon_rpc.unwrap().to_string(), "http://localhost:5052/");
            }
        }

        assert!(matches!(cli.kona.mode, CliNodeMode::Validator));
        assert_eq!(cli.kona.rpc.listen_port, 9546);
    }

    #[test]
    fn test_config_creation() {
        let args = vec![
            "rollup",
            "node",
            "--l1.eth",
            "http://localhost:8545",
            "--l1.beacon",
            "http://localhost:5052",
            "--chain",
            "10",
        ];

        let cli = RollupCli::try_parse_from(args).unwrap();
        let config = cli.create_kona_config().unwrap();

        assert_eq!(config.mode, NodeMode::Validator);
        assert_eq!(config.l1_eth_rpc.to_string(), "http://localhost:8545/");
        assert_eq!(config.l1_beacon.to_string(), "http://localhost:5052/");
    }

    #[test]
    fn test_validation_errors() {
        let args = vec!["rollup", "node", "--l1.beacon", "http://localhost:5052"];
        let cli = RollupCli::try_parse_from(args).unwrap();
        assert!(cli.validate().is_err());
    }

    #[test]
    fn test_port_conflict() {
        let args = vec![
            "rollup",
            "--kona.rpc.port",
            "8545",
            "node",
            "--l1.eth",
            "http://localhost:8545",
            "--l1.beacon",
            "http://localhost:5052",
        ];

        let cli = RollupCli::try_parse_from(args).unwrap();
        assert!(cli.validate().is_err());
    }
}
