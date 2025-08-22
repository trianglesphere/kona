//! Kona Node Execution Extension for Reth integration.
//!
//! This module provides the ExEx implementation that wraps kona-node-service
//! and integrates it with the Reth node through the ExEx interface.

use alloy_primitives::{BlockNumber, B256};
use futures::StreamExt;
use kona_node_service::RollupNode;
use kona_providers_local::{BufferedL2Provider, ChainStateEvent};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_types::NodeTypes;
use reth_primitives::{NodePrimitives, SealedBlock};
use reth_primitives_traits::AlloyBlockHeader;
use std::future::Future;
use tracing::{error, info};

/// Kona Node Execution Extension that wraps the RollupNode.
/// 
/// Node must implement FullNodeComponents and have Types that implement NodeTypes
/// with primitives that are NodePrimitives.
pub struct KonaNodeExEx<Node>
where
    Node: FullNodeComponents,
    Node::Types: NodeTypes,
    <Node::Types as NodeTypes>::Primitives: NodePrimitives,
{
    /// The underlying Kona rollup node.
    node: Option<RollupNode>,
    /// ExEx context for communication with Reth.
    ctx: ExExContext<Node>,
    /// Buffered provider for caching L2 chain state.
    provider: BufferedL2Provider,
}

impl<Node> KonaNodeExEx<Node>
where
    Node: FullNodeComponents,
    Node::Types: NodeTypes,
    <Node::Types as NodeTypes>::Primitives: NodePrimitives,
{
    /// Creates a new Kona Node ExEx from the given context.
    pub fn new(ctx: ExExContext<Node>) -> anyhow::Result<Self> {
        info!("Initializing Kona Node ExEx");

        // Create buffered provider for L2 chain state caching
        let url = "http://localhost:8545".parse()?;
        let alloy_provider = alloy_provider::RootProvider::new_http(url);
        let rollup_config = std::sync::Arc::new(Default::default());
        let inner =
            kona_providers_alloy::AlloyL2ChainProvider::new(alloy_provider, rollup_config, 100);
        let provider = BufferedL2Provider::new(inner, 1000, 64);

        Ok(Self {
            node: None, // TODO: Initialize RollupNode when proper configuration is available
            ctx,
            provider,
        })
    }

    /// Starts the ExEx and returns a future that runs indefinitely.
    pub fn start(mut self) -> impl Future<Output = anyhow::Result<()>>
    where
        Node::Types: NodePrimitives,
    {
        async move {
            info!("Starting Kona Node ExEx");

            // TODO: Start the actual RollupNode when proper configuration is available
            
            // Main ExEx processing loop
            let mut last_processed_height: Option<BlockNumber> = None;

            loop {
                tokio::select! {
                    // Process ExEx notifications from Reth  
                    notification = self.ctx.notifications.next() => {
                        if let Some(notification_result) = notification {
                            match notification_result {
                                Ok(notification) => {
                                    if let Err(e) = self.process_notification(notification, &mut last_processed_height).await {
                                        error!("Failed to process ExEx notification: {}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("Error receiving ExEx notification: {}", e);
                                }
                            }
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
        notification: ExExNotification<<Node::Types as NodeTypes>::Primitives>,
        last_processed_height: &mut Option<BlockNumber>,
    ) -> anyhow::Result<()>
    where
        <Node::Types as NodeTypes>::Primitives: NodePrimitives,
    {
        match notification {
            ExExNotification::ChainCommitted { new } => {
                let blocks = new.blocks();
                info!("Processing committed chain with {} blocks", blocks.len());

                // Extract block hashes from the chain
                let committed_hashes: Vec<B256> = blocks
                    .values()
                    .map(|block| B256::from(block.hash()))
                    .collect();

                // Get the last block by finding the highest block number
                if let Some((_, last_block)) = blocks.iter().max_by_key(|(number, _)| *number) {
                    let event = ChainStateEvent::ChainCommitted {
                        new_head: B256::from(last_block.hash()),
                        committed: committed_hashes,
                    };
                    self.provider.handle_chain_event(event).await?;

                    // Update last processed height
                    *last_processed_height = Some(last_block.number());

                    // Send FinishedHeight event back to Reth using NumHash (number, hash) tuple
                    let num_hash = (last_block.number(), last_block.hash()).into();
                    if let Err(e) = self.ctx.events.send(ExExEvent::FinishedHeight(num_hash)) {
                        error!("Failed to send FinishedHeight event: {}", e);
                        return Err(e.into());
                    }

                    info!("Processed chain up to block {}", last_block.number());
                }
            }

            ExExNotification::ChainReorged { old, new } => {
                let old_blocks = old.blocks();
                let new_blocks = new.blocks();
                info!(
                    "Processing chain reorg (old: {} blocks, new: {} blocks)",
                    old_blocks.len(),
                    new_blocks.len()
                );

                // Handle reorg in buffered provider
                let depth = old_blocks.len() as u64;
                
                // Get the last blocks from old and new chains
                let old_last = old_blocks.iter().max_by_key(|(number, _)| *number);
                let new_last = new_blocks.iter().max_by_key(|(number, _)| *number);
                
                if let (Some((_, old_last_block)), Some((_, new_last_block))) = (old_last, new_last) {
                    let event = ChainStateEvent::ChainReorged {
                        old_head: B256::from(old_last_block.hash()),
                        new_head: B256::from(new_last_block.hash()),
                        depth,
                    };
                    self.provider.handle_chain_event(event).await?;

                    // Update last processed height
                    *last_processed_height = Some(new_last_block.number());

                    // Send FinishedHeight event back to Reth
                    let num_hash = (new_last_block.number(), new_last_block.hash()).into();
                    if let Err(e) = self.ctx.events.send(ExExEvent::FinishedHeight(num_hash)) {
                        error!("Failed to send FinishedHeight event: {}", e);
                        return Err(e.into());
                    }

                    info!("Processed reorg up to block {}", new_last_block.number());
                }
            }

            ExExNotification::ChainReverted { old } => {
                let old_blocks = old.blocks();
                info!("Processing chain revert with {} blocks", old_blocks.len());

                // Handle revert in buffered provider
                let old_last = old_blocks.iter().max_by_key(|(number, _)| *number);
                let old_first = old_blocks.iter().min_by_key(|(number, _)| *number);
                
                if let (Some((_, old_last_block)), Some((_, old_first_block))) = (old_last, old_first) {
                    let reverted_hashes: Vec<B256> = old_blocks
                        .values()
                        .map(|block| B256::from(block.hash()))
                        .collect();

                    let event = ChainStateEvent::ChainReverted {
                        old_head: B256::from(old_last_block.hash()),
                        new_head: B256::from(old_first_block.hash()), // After revert, head goes back to before the reverted blocks
                        reverted: reverted_hashes,
                    };
                    self.provider.handle_chain_event(event).await?;

                    // Update last processed height to the block before the reverted chain
                    let new_height = old_first_block.number().saturating_sub(1);
                    *last_processed_height = Some(new_height);

                    // Send FinishedHeight event back to Reth
                    // Use a placeholder hash since we don't have the actual block at new_height
                    let num_hash = (new_height, old_first_block.hash()).into();
                    if let Err(e) = self.ctx.events.send(ExExEvent::FinishedHeight(num_hash)) {
                        error!("Failed to send FinishedHeight event: {}", e);
                        return Err(e.into());
                    }

                    info!("Processed revert back to block {}", new_height);
                }
            }
        }

        Ok(())
    }
}

/// Helper function to convert reth SealedBlock to information needed by the buffered provider.
fn extract_block_info(block: &SealedBlock) -> (BlockNumber, B256) {
    (block.number, B256::from(block.hash()))
}