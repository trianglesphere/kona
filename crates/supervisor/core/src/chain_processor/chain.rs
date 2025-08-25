use super::{
    handlers::{
        CrossSafeHandler, CrossUnsafeHandler, EventHandler, FinalizedHandler, InvalidationHandler,
        OriginHandler, ReplacementHandler, SafeBlockHandler, UnsafeBlockHandler,
    },
    state::{ProcessorState, ProcesssorStateUpdate},
};
use crate::{
    LogIndexer,
    event::ChainEvent,
    syncnode::{BlockProvider, ManagedNodeCommand},
};
use alloy_primitives::ChainId;
use kona_interop::InteropValidator;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DerivationStorage, HeadRefStorage, LogStorage, StorageRewinder};
use std::{fmt::Debug, sync::Arc};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Represents a task that processes chain events from a managed node.
/// It listens for events emitted by the managed node and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessor<P, W, V> {
    chain_id: ChainId,
    metrics_enabled: Option<bool>,

    // state
    state: ProcessorState,

    // Handlers for different types of chain events.
    unsafe_handler: UnsafeBlockHandler<P, W, V>,
    safe_handler: SafeBlockHandler<P, W, V>,
    origin_handler: OriginHandler<W>,
    invalidation_handler: InvalidationHandler<W>,
    replacement_handler: ReplacementHandler<P, W>,
    finalized_handler: FinalizedHandler<W>,
    cross_unsafe_handler: CrossUnsafeHandler,
    cross_safe_handler: CrossSafeHandler,
}

impl<P, W, V> ChainProcessor<P, W, V>
where
    P: BlockProvider + 'static,
    V: InteropValidator + 'static,
    W: LogStorage + DerivationStorage + HeadRefStorage + StorageRewinder + 'static,
{
    /// Creates a new [`ChainProcessor`].
    pub fn new(
        validator: Arc<V>,
        chain_id: ChainId,
        managed_node: Arc<P>,
        db_provider: Arc<W>,
        managed_node_sender: mpsc::Sender<ManagedNodeCommand>,
    ) -> Self {
        let log_indexer = Arc::new(LogIndexer::new(chain_id, managed_node, db_provider.clone()));

        let unsafe_handler = UnsafeBlockHandler::new(
            chain_id,
            validator.clone(),
            db_provider.clone(),
            log_indexer.clone(),
        );

        let safe_handler = SafeBlockHandler::new(
            chain_id,
            managed_node_sender.clone(),
            db_provider.clone(),
            validator,
            log_indexer.clone(),
        );

        let origin_handler =
            OriginHandler::new(chain_id, managed_node_sender.clone(), db_provider.clone());

        let invalidation_handler =
            InvalidationHandler::new(chain_id, managed_node_sender.clone(), db_provider.clone());

        let replacement_handler =
            ReplacementHandler::new(chain_id, log_indexer, db_provider.clone());

        let finalized_handler =
            FinalizedHandler::new(chain_id, managed_node_sender.clone(), db_provider.clone());
        let cross_unsafe_handler = CrossUnsafeHandler::new(chain_id, managed_node_sender.clone());
        let cross_safe_handler = CrossSafeHandler::new(chain_id, managed_node_sender);

        let super_head = db_provider
            .get_super_head()
            .map_err(|err| {
                warn!(
                    target: "supervisor::chain_processor",
                    chain_id = chain_id,
                    %err,
                    "Failed to get super head on chain processor initialization"
                );
            })
            .unwrap_or_default();

        Self {
            chain_id,
            metrics_enabled: None,

            state: ProcessorState { super_head, invalidated_block: None },

            // Handlers for different types of chain events.
            unsafe_handler,
            safe_handler,
            origin_handler,
            invalidation_handler,
            replacement_handler,
            finalized_handler,
            cross_unsafe_handler,
            cross_safe_handler,
        }
    }

    /// Enables metrics on the database environment.
    pub fn with_metrics(mut self) -> Self {
        self.metrics_enabled = Some(true);
        super::Metrics::init(self.chain_id);
        self
    }

    /// Handles a chain event by delegating it to the appropriate handler.
    pub async fn handle_event(&mut self, event: ChainEvent) {
        let (result, head_update) = match event {
            ChainEvent::UnsafeBlock { block } => (
                self.unsafe_handler.handle(block, &mut self.state).await,
                ProcesssorStateUpdate::LocalUnsafe,
            ),
            ChainEvent::DerivedBlock { derived_ref_pair } => (
                self.safe_handler.handle(derived_ref_pair, &mut self.state).await,
                ProcesssorStateUpdate::LocalSafe,
            ),
            ChainEvent::DerivationOriginUpdate { origin } => (
                self.origin_handler.handle(origin, &mut self.state).await,
                ProcesssorStateUpdate::L1Source,
            ),
            ChainEvent::InvalidateBlock { block } => (
                self.invalidation_handler.handle(block, &mut self.state).await,
                ProcesssorStateUpdate::InvalidateBlock,
            ),
            ChainEvent::BlockReplaced { replacement } => (
                self.replacement_handler.handle(replacement, &mut self.state).await,
                ProcesssorStateUpdate::BlockReplaced,
            ),
            ChainEvent::FinalizedSourceUpdate { finalized_source_block } => (
                self.finalized_handler.handle(finalized_source_block, &mut self.state).await,
                ProcesssorStateUpdate::Finalized, /* Finalized handler returns the derived block
                                                   * on success */
            ),
            ChainEvent::CrossUnsafeUpdate { block } => (
                self.cross_unsafe_handler.handle(block, &mut self.state).await,
                ProcesssorStateUpdate::CrossUnsafe,
            ),
            ChainEvent::CrossSafeUpdate { derived_ref_pair } => (
                self.cross_safe_handler.handle(derived_ref_pair, &mut self.state).await,
                ProcesssorStateUpdate::CrossSafe,
            ),
        };

        if let Err(err) = &result {
            debug!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                %err,
                ?event,
                "Failed to process event"
            );
        }

        if let Ok(block) = result {
            self.apply_state_update(head_update, block);
        }
    }

    fn apply_state_update(&mut self, update: ProcesssorStateUpdate, block: BlockInfo) {
        match update {
            ProcesssorStateUpdate::LocalUnsafe => {
                self.state.super_head.local_unsafe = block;
                self.log_state_update("LocalUnsafe", &block);
            }
            ProcesssorStateUpdate::LocalSafe => {
                self.state.super_head.local_safe = Some(block);
                self.log_state_update("LocalSafe", &block);
            }
            ProcesssorStateUpdate::CrossUnsafe => {
                self.state.super_head.cross_unsafe = Some(block);
                self.log_state_update("CrossUnsafe", &block);
            }
            ProcesssorStateUpdate::CrossSafe => {
                self.state.super_head.cross_safe = Some(block);
                self.log_state_update("CrossSafe", &block);
            }
            ProcesssorStateUpdate::Finalized => {
                self.state.super_head.finalized = Some(block);
                self.log_state_update("Finalized", &block);
            }
            ProcesssorStateUpdate::L1Source => {
                self.state.super_head.l1_source = Some(block);
                self.log_state_update("L1Source", &block);
            }
            ProcesssorStateUpdate::InvalidateBlock => {
                self.log_state_update("InvalidateBlock", &block);
            }
            ProcesssorStateUpdate::BlockReplaced => {
                self.log_state_update("BlockReplaced", &block);
            }
        }
    }

    fn log_state_update(&self, update: &str, block: &BlockInfo) {
        info!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            update_event = %update,
            block_number = block.number,
            block_hash = %block.hash,
            "Chain processor state updated."
        );
    }
}
