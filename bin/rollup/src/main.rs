//! Unified rollup binary with kona-node ExEx integration.

#![deny(missing_docs, unused_must_use, rust_2018_idioms)]

mod cli;
mod exex;

use clap::Parser;
use cli::RollupCli;
use tracing::{error, info};

/// Main entry point for the unified rollup binary.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing();

    let cli = RollupCli::parse();

    info!("Starting Kona Rollup Binary");

    if let Err(e) = cli.validate() {
        error!("CLI validation failed: {}", e);
        std::process::exit(1);
    }

    let kona_config = cli.create_kona_config()?;
    info!(
        "Created Kona configuration: mode={}, l1_eth={}, l1_beacon={}",
        kona_config.mode, kona_config.l1_eth_rpc, kona_config.l1_beacon
    );

    info!("✓ CLI parsing successful");
    info!("✓ Configuration validation passed");
    info!("✓ Ready for reth integration");

    Ok(())
}

/// Setup tracing with default settings.
fn setup_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,kona=debug,reth=info".into()),
        )
        .init();
}
