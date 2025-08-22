//! Unified rollup binary that integrates kona-node as an ExEx into op-reth.
//!
//! This binary follows the ExEx pattern to embed the Kona rollup node directly
//! into the op-reth execution client, reducing operational complexity and latency.
//!
//! ## Architecture
//!
//! The rollup binary is designed to be extremely simple:
//!
//! 1. Parse op-reth CLI arguments using reth's native CLI parser
//! 2. Build the op-reth node using the node builder
//! 3. Install kona-node as an ExEx named "KonaNode"
//! 4. Launch the combined system and wait for shutdown
//!
//! This approach eliminates the need for separate binaries and reduces the
//! operational burden on rollup operators.

#![doc(issue_tracker_base_url = "https://github.com/op-rs/kona/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(rust_2018_idioms)]

use rollup::{ExExContext, KonaNodeExEx};
use tokio::sync::mpsc;
use tracing::info;

/// Main entry point for the unified rollup binary.
///
/// ## Usage Pattern
///
/// This implementation follows the ExEx pattern described in EXEX.md:
///
/// ```rust,ignore
/// reth::cli::Cli::parse_args().run(|builder, _| async move {
///     let handle = builder
///         .node(OpNode::default())
///         .install_exex("KonaNode", move |ctx| async {
///             Ok(KonaNode::new(ctx)?.start())
///         })
///         .launch()
///         .await?;
///     handle.wait_for_node_exit().await
/// })
/// ```
///
/// ## Current Status
///
/// This is a placeholder implementation until proper reth and op-reth ExEx
/// dependencies are available in the workspace. The structure demonstrates
/// the intended pattern and can be easily updated once dependencies are resolved.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup tracing and signal handlers
    setup_tracing_and_signals();

    info!("Starting Kona Rollup Binary");
    info!("This binary integrates kona-node as an ExEx into op-reth");

    // TODO: Replace with actual reth CLI parsing once dependencies are available
    // reth::cli::Cli::parse_args().run(|builder, _| async move {
    //     let handle = builder
    //         .node(op_reth_node::OpNode::default())
    //         .install_exex("KonaNode", move |ctx| async {
    //             let kona_exex = KonaNodeExEx::new(ctx)?;
    //             Ok(kona_exex.start())
    //         })
    //         .launch()
    //         .await?;
    //     handle.wait_for_node_exit().await
    // }).await

    // Placeholder implementation showing the intended structure
    demo_exex_integration().await
}

/// Demonstrates the ExEx integration pattern until proper dependencies are available.
async fn demo_exex_integration() -> anyhow::Result<()> {
    info!("Running demo ExEx integration");

    // Create mock ExEx context
    let (events_tx, _events_rx) = mpsc::unbounded_channel();
    let (_notifications_tx, notifications_rx) = mpsc::unbounded_channel();

    let ctx = ExExContext { notifications: notifications_rx, events: events_tx };

    // Create and start the Kona Node ExEx
    info!("Creating Kona Node ExEx");
    let kona_exex = KonaNodeExEx::new(ctx)?;

    info!("Starting Kona Node ExEx");
    let exex_future = kona_exex.start();

    // In the real implementation, this would be handled by the reth node
    // For now, just demonstrate the structure
    tokio::select! {
        result = exex_future => {
            info!("ExEx completed: {:?}", result);
            result
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
            Ok(())
        }
    }
}

/// Setup tracing and signal handlers for better error reporting.
fn setup_tracing_and_signals() {
    // Initialize tracing with default settings
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,kona=debug".into()),
        )
        .init();

    info!("Tracing initialized");
}
