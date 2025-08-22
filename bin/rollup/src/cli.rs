//! CLI implementation for the rollup binary.

use crate::{flags::GlobalArgs, node_builder};
use anyhow::Result;
use clap::Parser;
use kona_cli::{
    LogConfig,
    node_config::{
        ForkOverrides, GlobalConfig, NodeCliConfig, P2PConfig, RpcConfig, SequencerConfig,
    },
};
use tracing::{error, info};

/// The rollup CLI.
#[derive(Parser, Clone, Debug)]
#[command(
    name = "rollup",
    about = "Unified rollup binary with kona-node ExEx integration",
    version
)]
pub struct Cli {
    /// Global arguments.
    #[command(flatten)]
    pub args: GlobalArgs,
}

impl Cli {
    /// Run the rollup binary.
    pub fn run(self) -> Result<()> {
        self.init_logs()?;

        if let Err(e) = self.validate_args() {
            error!("CLI validation failed: {}", e);
            return Err(anyhow::anyhow!(e));
        }

        let kona_config = self.args_to_config();
        Self::run_until_ctrl_c(self.run_async(kona_config))
    }

    /// Run the async portion of the rollup binary.
    async fn run_async(self, kona_config: NodeCliConfig) -> Result<()> {
        node_builder::run_op_reth_with_kona_exex(kona_config).await?;
        Ok(())
    }

    /// Initialize logging using LogConfig.
    fn init_logs(&self) -> Result<()> {
        // Use the same filter as kona-node for consistency
        let filter = tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("discv5=error".parse()?);

        LogConfig::new(self.args.log_args.clone()).init_tracing_subscriber(Some(filter))?;
        Ok(())
    }

    /// Run until ctrl-c is pressed.
    pub fn run_until_ctrl_c<F>(fut: F) -> Result<()>
    where
        F: std::future::Future<Output = Result<()>>,
    {
        let rt = Self::tokio_runtime()?;
        rt.block_on(fut)
    }

    /// Creates a new default tokio multi-thread [Runtime](tokio::runtime::Runtime)
    pub fn tokio_runtime() -> Result<tokio::runtime::Runtime> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Validate all CLI arguments.
    fn validate_args(&self) -> Result<(), String> {
        validate_args(&self.args)
    }

    /// Convert CLI arguments to NodeCliConfig.
    fn args_to_config(&self) -> NodeCliConfig {
        args_to_config(&self.args)
    }
}

/// Validate all CLI arguments.
fn validate_args(args: &GlobalArgs) -> Result<(), String> {
    // Validate P2P configuration
    args.p2p.validate()?;

    // Validate RPC configuration
    args.rpc.validate()?;

    // Validate sequencer configuration
    args.sequencer.validate()?;

    // Check for mode-specific requirements
    if args.mode == kona_cli::node_config::NodeMode::Sequencer &&
        args.sequencer.conductor_rpc.is_none()
    {
        tracing::warn!("Running in sequencer mode without conductor service");
    }

    Ok(())
}

/// Convert CLI arguments to NodeCliConfig.
fn args_to_config(args: &GlobalArgs) -> NodeCliConfig {
    NodeCliConfig {
        mode: args.mode,
        l1_eth_rpc: args.l1_eth_rpc.clone(),
        l1_trust_rpc: args.l1_trust_rpc,
        l1_beacon: args.l1_beacon.clone(),
        l2_trust_rpc: true, // Always true for ExEx
        l2_config_file: args.config_file.clone(),
        global: GlobalConfig {
            l2_chain_id: args.l2_chain_id.into(),
            fork_overrides: ForkOverrides::default(),
        },
        p2p: p2p_args_to_config(&args.p2p),
        rpc: rpc_args_to_config(&args.rpc),
        sequencer: sequencer_args_to_config(&args.sequencer),
    }
}

/// Convert P2P CLI arguments to P2PConfig.
fn p2p_args_to_config(args: &crate::flags::P2PArgs) -> P2PConfig {
    P2PConfig {
        no_discovery: args.no_discovery || args.p2p_disabled,
        priv_path: None, // Not exposed in simplified flags
        listen_ip: args.listen_ip,
        listen_tcp_port: args.listen_tcp_port,
        listen_udp_port: args.listen_udp_port,
        peers_lo: args.peers_lo,
        peers_hi: args.peers_hi,
        scoring: "light".to_string(), // Default scoring
        ban_enabled: false,           // Default to false
        unsafe_block_signer: None,    // Not exposed for security
    }
}

/// Convert RPC CLI arguments to RpcConfig.
fn rpc_args_to_config(args: &crate::flags::RpcArgs) -> RpcConfig {
    RpcConfig {
        disabled: args.rpc_disabled,
        listen_addr: args.addr,
        listen_port: args.port,
        enable_admin: args.enable_admin,
        admin_persistence: None, // Not exposed in simplified flags
        ws_enabled: args.ws_enabled,
        dev_enabled: args.dev_enabled,
    }
}

/// Convert Sequencer CLI arguments to SequencerConfig.
fn sequencer_args_to_config(args: &crate::flags::SequencerArgs) -> SequencerConfig {
    SequencerConfig {
        stopped: args.stopped,
        max_safe_lag: args.max_safe_lag,
        l1_confs: args.l1_confs,
        recover: args.recover,
        conductor_rpc: args.conductor_rpc.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kona_cli::node_config::NodeMode;

    #[test]
    fn test_args_to_config_default() {
        let args = GlobalArgs::default();
        let config = args_to_config(&args);
        assert_eq!(config.mode, NodeMode::Validator);
        assert_eq!(config.global.l2_chain_id, 10);
        assert!(config.l1_trust_rpc);
        assert!(config.l2_trust_rpc);
    }

    #[test]
    fn test_args_to_config_sequencer() {
        let mut args = GlobalArgs::default();
        args.mode = NodeMode::Sequencer;
        args.l2_chain_id = 8453;
        let config = args_to_config(&args);
        assert_eq!(config.mode, NodeMode::Sequencer);
        assert_eq!(config.global.l2_chain_id, 8453);
    }

    #[test]
    fn test_validate_args_success() {
        let args = GlobalArgs::default();
        assert!(validate_args(&args).is_ok());
    }

    #[test]
    fn test_validate_args_p2p_error() {
        let mut args = GlobalArgs::default();
        args.p2p.listen_tcp_port = 9000;
        args.p2p.listen_udp_port = 9000;
        args.p2p.p2p_disabled = false;
        assert!(validate_args(&args).is_err());
    }
}
