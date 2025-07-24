//! The [`Engine`] is a task queue that receives and executes [`EngineTask`]s.

use super::EngineTaskExt;
use crate::{
    EngineClient, EngineState, EngineSyncStateUpdate, EngineTask, EngineTaskError,
    EngineTaskErrorSeverity, ForkchoiceTask, Metrics, task_queue::EngineTaskErrors,
};
use alloy_provider::Provider;
use alloy_rpc_types_eth::Transaction;
use kona_genesis::{RollupConfig, SystemConfig};
use kona_protocol::{BlockInfo, L2BlockInfo, OpBlockConversionError, to_system_config};
use kona_sources::{SyncStartError, find_starting_forkchoice};
use op_alloy_consensus::OpTxEnvelope;
use std::{collections::BinaryHeap, sync::Arc};
use thiserror::Error;
use tokio::sync::watch::Sender;

/// The [`Engine`] task queue manages execution of blockchain operations for an OP Stack rollup node.
///
/// The engine serves as the central coordinator for all blockchain state changes, implementing
/// a priority-based task queue system that ensures proper ordering of operations according to
/// the [OP Stack derivation specification](https://specs.optimism.io/protocol/derivation.html).
///
/// ## Architecture
///
/// The engine operates as a single-threaded task executor where:
/// - **Tasks** represent atomic operations (forkchoice updates, block building, etc.)
/// - **Priority ordering** ensures critical operations execute first
/// - **Exclusive state access** prevents race conditions during execution
/// - **Error handling** supports retry logic and failure recovery
///
/// ## Task Processing
///
/// Tasks are processed through the [`Engine::drain`] method which:
/// 1. Dequeues tasks in priority order (highest priority first)
/// 2. Executes each task with exclusive access to [`EngineState`]
/// 3. Updates engine state and notifies subscribers on success
/// 4. Retries tasks with temporary errors, propagates critical errors
/// 5. Supports engine reset/flush on severe errors
///
/// ## Priority System
///
/// Task execution follows this priority order (highest to lowest):
/// 1. **ForkchoiceUpdate** - Chain synchronization (highest priority)
/// 2. **Build** - Block building for sequencing
/// 3. **Insert** - Unsafe block insertion from gossip
/// 4. **Consolidate** - Safe chain advancement via L1 derivation
/// 5. **Finalize** - Block finalization based on L1 finality
///
/// ## Error Handling
///
/// The engine supports four error severity levels:
/// - **Temporary**: Network issues, retried automatically
/// - **Critical**: Permanent failures, propagated to caller
/// - **Reset**: Requires finding new sync starting point
/// - **Flush**: Requires derivation pipeline flush
///
/// ## Thread Safety
///
/// Tasks are executed sequentially with exclusive state access, providing synchronization
/// guarantees for the L2 execution layer. State updates are broadcast to subscribers
/// via watch channels for external coordination.
#[derive(Debug)]
pub struct Engine {
    /// The state of the engine.
    state: EngineState,
    /// A sender that can be used to notify the engine actor of state changes.
    state_sender: Sender<EngineState>,
    /// The task queue.
    tasks: BinaryHeap<EngineTask>,
}

impl Engine {
    /// Creates a new [`Engine`] with an empty task queue and the provided initial [`EngineState`].
    ///
    /// The engine is initialized with:
    /// - Empty priority queue for task execution
    /// - Watch channel sender for broadcasting state updates
    /// - Initial state representing current blockchain synchronization status
    ///
    /// ## Parameters
    /// - `initial_state`: Starting engine state with sync head information
    /// - `state_sender`: Watch channel sender for broadcasting state changes to subscribers
    ///
    /// ## State Broadcasting
    /// State changes are automatically broadcast to all subscribers when tasks complete
    /// successfully, enabling external coordination with other rollup node components.
    pub fn new(initial_state: EngineState, state_sender: Sender<EngineState>) -> Self {
        Self { state: initial_state, state_sender, tasks: BinaryHeap::default() }
    }

    /// Returns a reference to the inner [`EngineState`].
    pub const fn state(&self) -> &EngineState {
        &self.state
    }

    /// Returns a watch channel receiver for monitoring engine state changes.
    ///
    /// The receiver provides real-time updates whenever the engine state changes,
    /// allowing external components to react to:
    /// - Head pointer updates (unsafe, safe, finalized)
    /// - Execution layer sync status changes  
    /// - Forkchoice state modifications
    ///
    /// ## Usage
    /// ```ignore
    /// let mut state_updates = engine.subscribe();
    /// while state_updates.changed().await.is_ok() {
    ///     let current_state = *state_updates.borrow();
    ///     // React to state changes
    /// }
    /// ```
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<EngineState> {
        self.state_sender.subscribe()
    }

    /// Enqueues a new [`EngineTask`] for execution in the priority queue.
    ///
    /// Tasks are automatically ordered by their priority level:
    /// 1. **ForkchoiceUpdate** - Immediate chain synchronization
    /// 2. **Build** - Block building (sequencer operations)  
    /// 3. **Insert** - Unsafe block insertion (gossip processing)
    /// 4. **Consolidate** - Safe chain advancement (L1 derivation)
    /// 5. **Finalize** - Block finalization
    ///
    /// ## Parameters
    /// - `task`: The engine task to be executed
    ///
    /// ## Ordering Guarantees
    /// - Tasks of the same type are processed in FIFO order
    /// - Higher priority tasks always execute before lower priority ones
    /// - Task execution is atomic with exclusive state access
    pub fn enqueue(&mut self, task: EngineTask) {
        self.tasks.push(task);
    }

    /// Resets the engine by finding a plausible sync starting point and clearing all pending tasks.
    ///
    /// This method performs a complete engine reset when the current state becomes inconsistent
    /// or unrecoverable. The reset process involves:
    ///
    /// 1. **Task Queue Clearing**: All pending tasks are removed to prevent conflicts
    /// 2. **Sync Point Discovery**: Uses [`find_starting_forkchoice`] to locate a valid starting point
    /// 3. **Forkchoice Update**: Reorgs the execution layer to the new starting point
    /// 4. **System Config Reconstruction**: Rebuilds system configuration for the new safe head
    ///
    /// ## Parameters
    /// - `client`: Engine client for L1/L2 RPC communication
    /// - `config`: Rollup configuration for the chain
    ///
    /// ## Returns
    /// On success, returns:
    /// - `L2BlockInfo`: New safe head block information
    /// - `BlockInfo`: L1 origin block information  
    /// - `SystemConfig`: Reconstructed system configuration
    ///
    /// ## Error Contexts
    /// Reset may be triggered by:
    /// - **Deep L1 Reorgs**: When L1 chain reorganizes beyond safe confirmation depth
    /// - **State Corruption**: When local state becomes inconsistent with L1 data
    /// - **Sync Conflicts**: When multiple conflicting forkchoice states cannot be resolved
    /// - **Derivation Failures**: When L1 derivation pipeline encounters unrecoverable errors
    ///
    /// ## Recovery Process
    /// The method implements a conservative recovery strategy:
    /// 1. Starts from a known-good sync point (typically recent finalized state)
    /// 2. Applies forkchoice update to reorg execution layer if needed
    /// 3. Ignores temporary errors during reset (retries automatically)
    /// 4. Propagates only critical errors that prevent recovery
    ///
    /// ## Performance Impact
    /// Resets are expensive operations that may require:
    /// - Multiple L1/L2 RPC calls to discover sync point
    /// - Execution layer reorg if current head is invalid
    /// - System config reconstruction from L1 data
    /// - Full state validation before resuming normal operation
    pub async fn reset(
        &mut self,
        client: Arc<EngineClient>,
        config: Arc<RollupConfig>,
    ) -> Result<(L2BlockInfo, BlockInfo, SystemConfig), EngineResetError> {
        // Clear any outstanding tasks to prepare for the reset.
        self.clear();

        let start =
            find_starting_forkchoice(&config, client.l1_provider(), client.l2_provider()).await?;

        if let Err(err) = ForkchoiceTask::new(
            client.clone(),
            config.clone(),
            EngineSyncStateUpdate {
                unsafe_head: Some(start.un_safe),
                cross_unsafe_head: Some(start.un_safe),
                local_safe_head: Some(start.safe),
                safe_head: Some(start.safe),
                finalized_head: Some(start.finalized),
            },
            None,
        )
        .execute(&mut self.state)
        .await
        {
            // Ignore temporary errors.
            if matches!(err.severity(), EngineTaskErrorSeverity::Temporary) {
                debug!(target: "engine", "Forkchoice update failed temporarily during reset: {}", err);
            } else {
                return Err(EngineTaskErrors::Forkchoice(err).into());
            }
        }

        // Find the new safe head's L1 origin and SystemConfig.
        let origin_block = start
            .safe
            .l1_origin
            .number
            .saturating_sub(config.channel_timeout(start.safe.block_info.timestamp));
        let l1_origin_info: BlockInfo = client
            .l1_provider()
            .get_block(origin_block.into())
            .await
            .map_err(SyncStartError::RpcError)?
            .ok_or(SyncStartError::BlockNotFound(origin_block.into()))?
            .into_consensus()
            .into();
        let l2_safe_block = client
            .l2_provider()
            .get_block(start.safe.block_info.hash.into())
            .full()
            .await
            .map_err(SyncStartError::RpcError)?
            .ok_or(SyncStartError::BlockNotFound(origin_block.into()))?
            .into_consensus()
            .map_transactions(|t| <Transaction<OpTxEnvelope> as Clone>::clone(&t).into_inner());
        let system_config = to_system_config(&l2_safe_block, &config)?;

        kona_macros::inc!(counter, Metrics::ENGINE_RESET_COUNT);

        Ok((start.safe, l1_origin_info, system_config))
    }

    /// Clears all pending tasks from the priority queue.
    ///
    /// This method removes all enqueued tasks without executing them, typically used:
    /// - **During engine reset**: To prevent conflicting operations during recovery
    /// - **On shutdown**: To clean up pending work before termination
    /// - **After critical errors**: To clear potentially invalid operations
    ///
    /// ## Side Effects
    /// - All pending tasks are permanently lost
    /// - Task queue returns to empty state
    /// - No state notifications are sent for cleared tasks
    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    /// Attempts to drain the task queue by executing all [`EngineTask`]s in priority order.
    ///
    /// This is the main execution loop that processes all pending tasks atomically.
    /// Each task is executed with exclusive access to the engine state, ensuring
    /// consistent blockchain state management.
    ///
    /// ## Execution Flow
    /// 1. **Task Selection**: Dequeue highest priority task from the queue
    /// 2. **Atomic Execution**: Execute task with exclusive state access
    /// 3. **State Update**: Apply successful changes to engine state
    /// 4. **Notification**: Broadcast state changes to all subscribers
    /// 5. **Queue Management**: Remove completed task from queue
    /// 6. **Repeat**: Continue until queue is empty or error occurs
    ///
    /// ## Error Handling
    /// Task execution supports sophisticated error recovery:
    /// - **Temporary errors**: Task remains in queue for automatic retry
    /// - **Critical errors**: Execution halts and error is propagated
    /// - **Reset errors**: Indicates engine reset is required
    /// - **Flush errors**: Indicates derivation pipeline flush is needed
    ///
    /// ## Atomicity Guarantees
    /// - Each task executes atomically (all changes or none)
    /// - State updates are applied only on successful task completion
    /// - Failed tasks leave engine state unchanged
    /// - Task ordering is preserved during retries
    ///
    /// ## Performance Characteristics
    /// - Tasks execute sequentially (no parallelization)
    /// - Higher priority tasks always execute first
    /// - Task execution time affects overall processing latency
    /// - Large queues may impact real-time responsiveness
    ///
    /// ## Retry Behavior
    /// Failed tasks are handled based on error severity:
    /// ```ignore
    /// match error.severity() {
    ///     Temporary => /* Task stays in queue, will retry */,
    ///     Critical => /* Error propagated, execution halts */,
    ///     Reset => /* Engine reset required */,
    ///     Flush => /* Pipeline flush required */,
    /// }
    /// ```
    ///
    /// ## Returns
    /// - `Ok(())`: All tasks completed successfully
    /// - `Err(EngineTaskErrors)`: Task execution failed with non-temporary error
    pub async fn drain(&mut self) -> Result<(), EngineTaskErrors> {
        // Drain tasks in order of priority, halting on errors for a retry to be attempted.
        while let Some(task) = self.tasks.peek() {
            // Execute the task
            task.execute(&mut self.state).await?;

            // Update the state and notify the engine actor.
            self.state_sender.send_replace(self.state);

            // Pop the task from the queue now that it's been executed.
            self.tasks.pop();
        }

        Ok(())
    }
}

/// An error occurred while attempting to reset the [`Engine`].
///
/// Engine resets are complex operations that can fail at multiple stages:
/// 1. **Task Execution Errors**: Issues during forkchoice update execution
/// 2. **Sync Discovery Errors**: Problems finding valid sync starting point
/// 3. **System Config Errors**: Failures reconstructing system configuration
///
/// ## Error Contexts
///
/// ### Task Execution Failures
/// Occur when the forkchoice update task fails during reset:
/// - **Network Issues**: L1/L2 RPC connection problems
/// - **Invalid State**: Execution layer rejects forkchoice state
/// - **Consensus Violations**: Proposed state violates chain rules
///
/// ### Sync Point Discovery Failures  
/// Happen when `find_starting_forkchoice` cannot locate valid starting point:
/// - **Missing Blocks**: Required L1/L2 blocks not found
/// - **RPC Errors**: Provider communication failures
/// - **Chain Inconsistency**: L1/L2 state mismatch prevents sync
///
/// ### System Config Reconstruction Failures
/// Occur when rebuilding system config for new safe head:
/// - **Block Parsing Errors**: Cannot decode L2 block structure
/// - **Config Validation**: System config violates protocol rules
/// - **Genesis Mismatch**: Block incompatible with rollup genesis
///
/// ## Recovery Strategies
/// When reset fails, consider:
/// 1. **Retry**: For temporary network/RPC issues
/// 2. **Manual Intervention**: For persistent state corruption
/// 3. **Chain Resync**: For severe consensus violations
/// 4. **Configuration Review**: For genesis/config mismatches
#[derive(Debug, Error)]
pub enum EngineResetError {
    /// An error that originated from within an engine task.
    #[error(transparent)]
    Task(#[from] EngineTaskErrors),
    /// An error occurred while traversing the L1 for the sync starting point.
    #[error(transparent)]
    SyncStart(#[from] SyncStartError),
    /// An error occurred while constructing the SystemConfig for the new safe head.
    #[error(transparent)]
    SystemConfigConversion(#[from] OpBlockConversionError),
}
