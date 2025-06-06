use crate::{
    LogIndexer,
    syncnode::{ManagedNodeProvider, NodeEvent},
};
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{LogStorageWriter, SafetyHeadRefStorage};
use kona_supervisor_types::BlockReplacement;
use std::{fmt::Debug, sync::Arc};
use op_alloy_consensus::interop::SafetyLevel;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Represents a task that processes chain events from a managed node.
/// It listens for events emitted by the managed node and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessorTask<P, W> {
    state_manager: Arc<W>,
    
    log_indexer: Arc<LogIndexer<P, W>>,

    cancel_token: CancellationToken,

    /// The channel for receiving node events.
    event_rx: mpsc::Receiver<NodeEvent>,
}

impl<P, W> ChainProcessorTask<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: LogStorageWriter + SafetyHeadRefStorage + 'static,
{
    /// Creates a new [`ChainProcessorTask`].
    pub fn new(
        managed_node: Arc<P>,
        state_manager: Arc<W>,
        cancel_token: CancellationToken,
        event_rx: mpsc::Receiver<NodeEvent>,
    ) -> Self {
        Self {
            cancel_token,
            event_rx,
            state_manager: state_manager.clone(),
            log_indexer: Arc::from(LogIndexer::new(managed_node, state_manager)),
        }
    }

    /// Runs the chain processor task, which listens for events and processes them.
    /// This method will run indefinitely until the cancellation token is triggered.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                maybe_event = self.event_rx.recv() => {
                    if let Some(event) = maybe_event {
                        self.handle_event(event).await;
                    }
                }
                _ = self.cancel_token.cancelled() => break,
            }
        }
    }

    async fn handle_event(&self, event: NodeEvent) {
        match event {
            NodeEvent::UnsafeBlock { block } => self.handle_unsafe_event(block).await,
            NodeEvent::DerivedBlock { derived_ref_pair } => {
                self.handle_safe_event(derived_ref_pair).await
            }
            NodeEvent::BlockReplaced { replacement } => {
                self.handle_block_replacement(replacement).await
            }
            NodeEvent::Reset => self.handle_reset_event().await,
        }
    }

    async fn handle_block_replacement(&self, _replacement: BlockReplacement) {
        // Logic to handle block replacement
    }

    async fn handle_safe_event(&self, _derived_ref_pair: DerivedRefPair) {
        // Logic to handle safe events
    }

    async fn handle_unsafe_event(&self, block_info: BlockInfo) {
        info!(
            target: "chain_processor",
            block_number = block_info.number,
            "Processing unsafe block"
        );
        if let Err(err) = self.log_indexer.process_and_store_logs(&block_info).await {
            error!(
                target: "chain_processor",
                block_number = block_info.number,
                %err,
                "Failed to process unsafe block"
            );
            // TODO: take next action based on the error
        }
    }

    // reset_full_range() ->
    // - Find out start and end for reset
    // - Start = GetAnchor point(First Cross Safe Block) from DB
    // - End = Latest Local Safe from DB
    // - Consistency check applied(BlockRefByNumber) from managed node and check hash
    // - First check End block is consistent
    // - Check Start is inconsistent - stop and cannot proceed further.
    // - Use bisection and find out the last consistent block
    // - from last_consistent_block recalculate safe head refs
    
    async fn handle_reset_event(&self) {
        // whenever we get reset signal from the node we need to find the last consistent block.
        // Based on the consistent block we need to re-calculate the safety head references and sent 
        // back those to the managed node.
        // Finding the consistent block might require a full range search from the start to the end.
        // That requires a bisection algorithm to find out the target block
        // We will just check if the Latest Local Safe block is conistent here, if not will chose the anchor point.
        // TODO: check and implement bisection to find out latest consistent block
        
        
        // Step 1: Get the safety head ref at LocalSafe level
        let block_info = match self.state_manager.get_safety_head_ref(SafetyLevel::LocalSafe) {
            Ok(info) => info,
            Err(err) => {
                error!(
                target: "chain_processor",
                %err,
                "Failed to get LocalSafe head ref"
            );
                return;
            }
        };

        // Step 2: Call managed_node.get_block_ref(block_info)
        let block_from_managed_node  = self.managed_node.get_block_ref(block_info).await {
            Ok(block_ref) => {
                info!(?block_ref, "Successfully fetched block reference");
                // You can proceed with block_ref if needed...
            }
            Err(err) => {
                error!(?err, ?block_info, "Failed to get block reference for given BlockInfo");
                return;
            }
        };
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncnode::{ManagedNodeError, NodeEvent, NodeSubscriber, ReceiptProvider};
    use alloy_primitives::B256;
    use async_trait::async_trait;
    use kona_supervisor_storage::{LogStorageWriter, StorageError};
    use kona_supervisor_types::{Log, Receipts};
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    use tokio::sync::mpsc;

    #[derive(Debug)]
    struct MockNode;

    #[async_trait]
    impl NodeSubscriber for MockNode {
        async fn start_subscription(
            &self,
            _event_tx: mpsc::Sender<NodeEvent>,
        ) -> Result<(), ManagedNodeError> {
            Ok(())
        }
    }
    #[async_trait]
    impl ReceiptProvider for MockNode {
        async fn fetch_receipts(&self, _block_hash: B256) -> Result<Receipts, ManagedNodeError> {
            Ok(vec![])
        }
    }

    #[derive(Debug)]
    struct MockStorage {
        called: Arc<AtomicBool>,
    }

    impl LogStorageWriter for MockStorage {
        fn store_block_logs(
            &self,
            _block: &BlockInfo,
            _logs: Vec<Log>,
        ) -> Result<(), StorageError> {
            self.called.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_handle_unsafe_event_triggers() {
        let called = Arc::new(AtomicBool::new(false));

        let node = Arc::new(MockNode);
        let writer = Arc::new(MockStorage { called: Arc::clone(&called) });

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let task = ChainProcessorTask::new(node, writer, cancel_token.clone(), rx);

        // Send unsafe block event
        let block =
            BlockInfo { number: 123, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 0 };

        tx.send(NodeEvent::UnsafeBlock { block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();

        assert!(called.load(Ordering::SeqCst));
    }
}
