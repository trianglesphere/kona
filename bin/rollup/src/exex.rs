//! Kona Node Execution Extension for Reth integration.
//!
//! This module provides the ExEx implementation that wraps kona-node-service
//! and integrates it with the Reth node through the ExEx interface.

use alloy_primitives::BlockNumber;
use kona_node_service::RollupNode;
use kona_providers_local::{BufferedL2Provider, ChainStateEvent};
use std::future::Future;
use tokio::sync::mpsc;
use tracing::{error, info};

/// Mock ExEx types until proper reth dependencies are available
/// TODO: Replace with actual reth-exex types when dependencies are resolved

/// Mock ExEx context containing channels for communication with Reth
pub struct ExExContext {
    /// Channel for receiving notifications from Reth
    pub notifications: mpsc::UnboundedReceiver<ExExNotification>,
    /// Channel for sending events back to Reth
    pub events: mpsc::UnboundedSender<ExExEvent>,
}

/// Mock ExEx notification types from Reth
#[derive(Debug)]
pub enum ExExNotification {
    /// New chain has been committed
    ChainCommitted { new: Vec<MockBlock> },
    /// Chain has been reorged
    ChainReorged { old: Vec<MockBlock>, new: Vec<MockBlock> },
    /// Chain has been reverted
    ChainReverted { old: Vec<MockBlock> },
}

/// Mock ExEx events sent back to Reth
#[derive(Debug)]
pub enum ExExEvent {
    /// Indicates processing has finished up to this height
    FinishedHeight(BlockNumber),
}

/// Mock block type
#[derive(Debug)]
pub struct MockBlock {
    /// Block number
    pub number: BlockNumber,
    /// Block hash
    pub hash: alloy_primitives::B256,
}

/// Kona Node Execution Extension that wraps the RollupNode.
pub struct KonaNodeExEx {
    /// The underlying Kona rollup node.
    node: Option<RollupNode>,
    /// Channel for receiving ExEx notifications.
    notifications: mpsc::UnboundedReceiver<ExExNotification>,
    /// Channel for sending ExEx events back to Reth.
    events: mpsc::UnboundedSender<ExExEvent>,
    /// Buffered provider for caching L2 chain state.
    provider: BufferedL2Provider,
}

impl KonaNodeExEx {
    /// Creates a new Kona Node ExEx from the given context.
    pub fn new(ctx: ExExContext) -> anyhow::Result<Self> {
        info!("Initializing Kona Node ExEx");

        // For now, we'll create a placeholder setup until proper integration is available
        // TODO: Replace with actual RollupNode construction when dependencies are resolved

        // Create a mock buffered provider
        let url = "http://localhost:8545".parse()?;
        let alloy_provider = alloy_provider::RootProvider::new_http(url);
        let rollup_config = std::sync::Arc::new(Default::default());
        let inner =
            kona_providers_alloy::AlloyL2ChainProvider::new(alloy_provider, rollup_config, 100);
        let provider = BufferedL2Provider::new(inner, 1000, 64);

        Ok(Self {
            node: None, // Placeholder until proper construction
            notifications: ctx.notifications,
            events: ctx.events,
            provider,
        })
    }

    /// Starts the ExEx and returns a future that runs indefinitely.
    pub fn start(mut self) -> impl Future<Output = anyhow::Result<()>> {
        async move {
            info!("Starting Kona Node ExEx");

            // TODO: Start the actual RollupNode when dependencies are resolved
            // For now, just run the ExEx processing loop

            // Main ExEx processing loop
            let mut last_processed_height: Option<BlockNumber> = None;

            loop {
                tokio::select! {
                    // Process ExEx notifications from Reth
                    Some(notification) = self.notifications.recv() => {
                        if let Err(e) = self.process_notification(notification, &mut last_processed_height).await {
                            error!("Failed to process ExEx notification: {}", e);
                        }
                    }

                    // Handle shutdown signal
                    _ = tokio::signal::ctrl_c() => {
                        info!("Received shutdown signal");
                        break;
                    }
                }
            }

            info!("Kona Node ExEx shutting down");
            Ok(())
        }
    }

    /// Process an ExEx notification from Reth.
    async fn process_notification(
        &mut self,
        notification: ExExNotification,
        last_processed_height: &mut Option<BlockNumber>,
    ) -> anyhow::Result<()> {
        match notification {
            ExExNotification::ChainCommitted { new } => {
                info!("Processing committed chain from block {}", new.first().unwrap().number);

                // Convert to chain state event and handle in buffered provider
                let event = ChainStateEvent::ChainCommitted {
                    new_head: new.last().unwrap().hash,
                    committed: new.iter().map(|b| b.hash).collect(),
                };
                self.provider.handle_chain_event(event).await?;

                // Update last processed height
                if let Some(last_block) = new.last() {
                    *last_processed_height = Some(last_block.number);

                    // Send FinishedHeight event back to Reth to indicate we've processed this block
                    self.events.send(ExExEvent::FinishedHeight(last_block.number))?;

                    info!("Processed chain up to block {}", last_block.number);
                }
            }

            ExExNotification::ChainReorged { old, new } => {
                info!(
                    "Processing chain reorg from block {} to {}",
                    old.first().unwrap().number,
                    new.first().unwrap().number
                );

                // Handle reorg in buffered provider
                let depth = old.len() as u64;
                let event = ChainStateEvent::ChainReorged {
                    old_head: old.last().unwrap().hash,
                    new_head: new.last().unwrap().hash,
                    depth,
                };
                self.provider.handle_chain_event(event).await?;

                // Update last processed height
                if let Some(last_block) = new.last() {
                    *last_processed_height = Some(last_block.number);

                    // Send FinishedHeight event back to Reth
                    self.events.send(ExExEvent::FinishedHeight(last_block.number))?;

                    info!("Processed reorg up to block {}", last_block.number);
                }
            }

            ExExNotification::ChainReverted { old } => {
                info!("Processing chain revert from block {}", old.first().unwrap().number);

                // Handle revert in buffered provider
                let event = ChainStateEvent::ChainReverted {
                    old_head: old.last().unwrap().hash,
                    new_head: old.first().unwrap().hash, /* After revert, head goes back to
                                                          * before the reverted blocks */
                    reverted: old.iter().map(|b| b.hash).collect(),
                };
                self.provider.handle_chain_event(event).await?;

                // Update last processed height to the block before the reverted chain
                if let Some(first_reverted) = old.first() {
                    let new_height = first_reverted.number.saturating_sub(1);
                    *last_processed_height = Some(new_height);

                    // Send FinishedHeight event back to Reth
                    self.events.send(ExExEvent::FinishedHeight(new_height))?;

                    info!("Processed revert back to block {}", new_height);
                }
            }
        }

        Ok(())
    }
}
