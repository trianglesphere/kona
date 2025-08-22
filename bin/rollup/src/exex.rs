//! Kona Node Execution Extension for reth integration.

use alloy_primitives::{B256, BlockNumber};
use futures::StreamExt;
use kona_cli::NodeCliConfig;
use kona_providers_local::{BufferedL2Provider, BufferedProviderError, ChainStateEvent};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_types::NodeTypes;
use reth_primitives::NodePrimitives;
use std::future::Future;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Errors for Kona ExEx operations.
#[derive(Debug, thiserror::Error)]
pub enum KonaExExError {
    #[error("Node service error: {0}")]
    NodeService(String),
    #[error("Provider error: {0}")]
    Provider(#[from] BufferedProviderError),
    #[error("ExEx error: {0}")]
    ExEx(String),
}

/// Kona Node ExEx that processes chain events.
#[derive(Debug)]
pub struct KonaNodeExEx<Node>
where
    Node: FullNodeComponents,
    Node::Types: NodeTypes,
    <Node::Types as NodeTypes>::Primitives: NodePrimitives,
{
    /// ExEx context for communication with reth.
    ctx: ExExContext<Node>,
    /// Buffered provider for L2 chain state caching.
    provider: BufferedL2Provider,
    /// Cancellation token for shutdown coordination.
    shutdown: CancellationToken,
}

impl<Node> KonaNodeExEx<Node>
where
    Node: FullNodeComponents,
    Node::Types: NodeTypes,
    <Node::Types as NodeTypes>::Primitives: NodePrimitives,
{
    /// Creates a new Kona Node ExEx.
    pub fn new(ctx: ExExContext<Node>) -> Result<Self, KonaExExError> {
        Self::new_with_config(ctx, NodeCliConfig::default())
    }

    /// Creates a new Kona Node ExEx with configuration.
    pub fn new_with_config(
        ctx: ExExContext<Node>,
        config: NodeCliConfig,
    ) -> Result<Self, KonaExExError> {
        info!("Initializing Kona Node ExEx with mode: {}", config.mode);

        let provider = Self::create_provider(&config)?;

        Ok(Self { ctx, provider, shutdown: CancellationToken::new() })
    }

    /// Creates buffered provider for L2 chain state caching.
    fn create_provider(config: &NodeCliConfig) -> Result<BufferedL2Provider, KonaExExError> {
        let alloy_provider = alloy_provider::RootProvider::new_http(config.l1_eth_rpc.clone());
        let rollup_config = std::sync::Arc::new(Default::default());
        let inner =
            kona_providers_alloy::AlloyL2ChainProvider::new(alloy_provider, rollup_config, 100);
        Ok(BufferedL2Provider::new(inner, 1000, 64))
    }

    /// Starts the ExEx main loop.
    pub fn start(mut self) -> impl Future<Output = Result<(), KonaExExError>>
    where
        Node::Types: NodePrimitives,
    {
        async move {
            info!("Starting Kona Node ExEx");

            let mut last_processed_height: Option<BlockNumber> = None;

            loop {
                tokio::select! {
                    notification = self.ctx.notifications.next() => {
                        if let Some(notification_result) = notification {
                            match notification_result {
                                Ok(notification) => {
                                    if let Err(e) = self.handle_notification(notification, &mut last_processed_height).await {
                                        error!("Failed to process notification: {}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("Error receiving notification: {}", e);
                                }
                            }
                        }
                    }
                    _ = self.shutdown.cancelled() => {
                        info!("Received shutdown signal");
                        break;
                    }
                    _ = tokio::signal::ctrl_c() => {
                        info!("Received ctrl-c, shutting down");
                        break;
                    }
                }
            }

            info!("Kona Node ExEx shutting down");
            Ok(())
        }
    }

    /// Processes an ExEx notification from reth.
    async fn handle_notification(
        &mut self,
        notification: ExExNotification<<Node::Types as NodeTypes>::Primitives>,
        last_processed_height: &mut Option<BlockNumber>,
    ) -> Result<(), KonaExExError>
    where
        <Node::Types as NodeTypes>::Primitives: NodePrimitives,
    {
        let event = self.convert_notification_to_event(&notification)?;
        self.provider.handle_chain_event(event).await?;

        let (height, hash, description) = match notification {
            ExExNotification::ChainCommitted { new } => {
                let blocks = new.blocks();
                info!("Processing {} committed blocks", blocks.len());

                let (height, block) =
                    blocks.iter().max_by_key(|(number, _)| *number).ok_or_else(|| {
                        KonaExExError::ExEx("No blocks in committed chain".to_string())
                    })?;
                (*height, block.hash(), "committed")
            }
            ExExNotification::ChainReorged { new, .. } => {
                let blocks = new.blocks();
                info!("Processing reorg to {} blocks", blocks.len());

                let (height, block) = blocks
                    .iter()
                    .max_by_key(|(number, _)| *number)
                    .ok_or_else(|| KonaExExError::ExEx("No blocks in reorged chain".to_string()))?;
                (*height, block.hash(), "reorged")
            }
            ExExNotification::ChainReverted { old } => {
                let blocks = old.blocks();
                info!("Processing {} reverted blocks", blocks.len());

                let (first_height, first_block) =
                    blocks.iter().min_by_key(|(number, _)| *number).ok_or_else(|| {
                        KonaExExError::ExEx("No blocks in reverted chain".to_string())
                    })?;
                (first_height.saturating_sub(1), first_block.hash(), "reverted")
            }
        };

        *last_processed_height = Some(height);

        let num_hash = (height, hash).into();
        self.ctx
            .events
            .send(ExExEvent::FinishedHeight(num_hash))
            .map_err(|e| KonaExExError::ExEx(format!("Failed to send FinishedHeight: {}", e)))?;

        info!("Processed {} chain up to block {}", description, height);
        Ok(())
    }

    /// Converts ExExNotification to ChainStateEvent.
    fn convert_notification_to_event(
        &self,
        notification: &ExExNotification<<Node::Types as NodeTypes>::Primitives>,
    ) -> Result<ChainStateEvent, KonaExExError> {
        match notification {
            ExExNotification::ChainCommitted { new } => {
                let blocks = new.blocks();
                let committed_hashes: Vec<B256> =
                    blocks.values().map(|b| B256::from(b.hash())).collect();
                let new_head = blocks
                    .iter()
                    .max_by_key(|(n, _)| *n)
                    .map(|(_, b)| B256::from(b.hash()))
                    .ok_or_else(|| {
                        KonaExExError::ExEx("No blocks in committed chain".to_string())
                    })?;

                Ok(ChainStateEvent::ChainCommitted { new_head, committed: committed_hashes })
            }
            ExExNotification::ChainReorged { old, new } => {
                let old_head = old
                    .blocks()
                    .iter()
                    .max_by_key(|(n, _)| *n)
                    .map(|(_, b)| B256::from(b.hash()))
                    .ok_or_else(|| KonaExExError::ExEx("No old blocks in reorg".to_string()))?;
                let new_head = new
                    .blocks()
                    .iter()
                    .max_by_key(|(n, _)| *n)
                    .map(|(_, b)| B256::from(b.hash()))
                    .ok_or_else(|| KonaExExError::ExEx("No new blocks in reorg".to_string()))?;

                Ok(ChainStateEvent::ChainReorged {
                    old_head,
                    new_head,
                    depth: old.blocks().len() as u64,
                })
            }
            ExExNotification::ChainReverted { old } => {
                let blocks = old.blocks();
                let old_head = blocks
                    .iter()
                    .max_by_key(|(n, _)| *n)
                    .map(|(_, b)| B256::from(b.hash()))
                    .ok_or_else(|| KonaExExError::ExEx("No old head in revert".to_string()))?;
                let new_head = blocks
                    .iter()
                    .min_by_key(|(n, _)| *n)
                    .map(|(_, b)| B256::from(b.hash()))
                    .ok_or_else(|| KonaExExError::ExEx("No new head in revert".to_string()))?;
                let reverted: Vec<B256> = blocks.values().map(|b| B256::from(b.hash())).collect();

                Ok(ChainStateEvent::ChainReverted { old_head, new_head, reverted })
            }
        }
    }
}
