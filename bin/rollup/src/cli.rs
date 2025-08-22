//! CLI for the unified rollup binary.

use crate::{RollupConfig, RollupResult, RollupError};
use clap::{Args, Parser, Subcommand};
use kona_cli::cli_styles;
use std::{net::SocketAddr, path::PathBuf};
use url::Url;

/// Version information for the rollup binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Subcommands for the rollup CLI.
#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    /// Run the unified rollup service.
    #[command(alias = "run", alias = "start")]
    Node(NodeCommand),
    /// Display version information.
    #[command(alias = "v")]
    Version,
}

/// The main CLI struct for the rollup binary.
#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version = VERSION,
    about = "Kona Rollup - Unified OP Stack Binary",
    long_about = "A minimal binary that launches the unified rollup service.",
    styles = cli_styles(),
)]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub subcommand: Commands,

    /// Global arguments.
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Global arguments shared across all subcommands.
#[derive(Args, Debug, Clone)]
pub struct GlobalArgs {
    /// Enable verbose logging.
    #[arg(short = 'v', long = "verbose", global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Disable colored output.
    #[arg(long = "no-color", global = true)]
    pub no_color: bool,

    /// Configuration file path.
    #[arg(long = "config", global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Data directory.
    #[arg(long = "datadir", global = true, value_name = "DIR")]
    pub datadir: Option<PathBuf>,

    /// Chain ID.
    #[arg(long = "chain", global = true, value_name = "CHAIN_ID", default_value = "10")]
    pub chain: String,
}

/// Node subcommand for running the unified rollup service.
#[derive(Args, Debug, Clone)]
pub struct NodeCommand {
    /// L1 RPC URL.
    #[arg(long = "l1-rpc-url", value_name = "URL", help = "L1 RPC endpoint")]
    pub l1_rpc_url: Url,

    /// L2 RPC listen address.
    #[arg(long = "l2-rpc-addr", value_name = "ADDR", default_value = "127.0.0.1:8545", help = "L2 RPC listen address")]
    pub l2_rpc_addr: SocketAddr,

    /// Engine API listen address.
    #[arg(long = "engine-addr", value_name = "ADDR", default_value = "127.0.0.1:8551", help = "Engine API listen address")]
    pub engine_addr: SocketAddr,

    /// P2P listen address.
    #[arg(long = "p2p-addr", value_name = "ADDR", default_value = "0.0.0.0:30303", help = "P2P listen address")]
    pub p2p_addr: SocketAddr,

    /// JWT secret file path.
    #[arg(long = "jwt-secret", value_name = "FILE", help = "Path to JWT secret file")]
    pub jwt_secret: Option<PathBuf>,

    /// Enable metrics.
    #[arg(long = "metrics", help = "Enable metrics server")]
    pub metrics: bool,

    /// Metrics listen address.
    #[arg(long = "metrics-addr", value_name = "ADDR", default_value = "127.0.0.1:9090", help = "Metrics listen address")]
    pub metrics_addr: SocketAddr,
}

impl Cli {
    /// Parse CLI arguments and validate configuration.
    pub fn parse_args() -> RollupResult<Self> {
        let cli = Self::parse();
        cli.validate()?;
        Ok(cli)
    }

    /// Validate the CLI configuration.
    pub fn validate(&self) -> RollupResult<()> {
        if let Commands::Node(node_cmd) = &self.subcommand {
            // Validate JWT secret exists if provided
            if let Some(jwt_path) = &node_cmd.jwt_secret {
                if !jwt_path.exists() {
                    return Err(RollupError::Config(format!(
                        "JWT secret file does not exist: {}", 
                        jwt_path.display()
                    )));
                }
            }
        }
        Ok(())
    }

    /// Convert CLI arguments to a unified rollup configuration.
    pub fn into_config(self) -> RollupResult<RollupConfig> {
        match self.subcommand {
            Commands::Node(node_cmd) => {
                RollupConfig::from_cli(self.global, node_cmd)
            }
            Commands::Version => {
                println!("kona-rollup {}", VERSION);
                std::process::exit(0);
            }
        }
    }

    /// Initialize logging based on the global arguments.
    pub fn init_logging(&self) -> RollupResult<()> {
        let log_level = match self.global.verbose {
            0 => "info",
            1 => "debug", 
            _ => "trace",
        };

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level))
            )
            .with_ansi(!self.global.no_color);

        subscriber.init();
        Ok(())
    }
}

impl Default for NodeCommand {
    fn default() -> Self {
        Self {
            l1_rpc_url: "http://localhost:8545".parse().expect("valid URL"),
            l2_rpc_addr: "127.0.0.1:8545".parse().expect("valid address"),
            engine_addr: "127.0.0.1:8551".parse().expect("valid address"),
            p2p_addr: "0.0.0.0:30303".parse().expect("valid address"),
            jwt_secret: None,
            metrics: false,
            metrics_addr: "127.0.0.1:9090".parse().expect("valid address"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_basic() {
        let args = vec![
            "rollup", "node",
            "--l1-rpc-url", "http://localhost:8545",
        ];

        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(cli.subcommand, Commands::Node(_)));
    }

    #[test]
    fn test_valid_configuration() {
        let args = vec![
            "rollup", "node",
            "--l1-rpc-url", "http://localhost:8545",
            "--l2-rpc-addr", "127.0.0.1:9545",
        ];

        let cli = Cli::try_parse_from(args).unwrap();
        let result = cli.validate();
        assert!(result.is_ok());
    }
}