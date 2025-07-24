//! Tasks sent to the [`Engine`] for execution.
//!
//! [`Engine`]: crate::Engine

use super::{BuildTask, ConsolidateTask, FinalizeTask, ForkchoiceTask, InsertTask};
use crate::{
    BuildTaskError, ConsolidateTaskError, EngineState, FinalizeTaskError, ForkchoiceTaskError,
    InsertTaskError,
};
use async_trait::async_trait;
use derive_more::Display;
use std::cmp::Ordering;
use thiserror::Error;

/// The severity of an engine task error, determining how the error is handled during task execution.
///
/// The engine's error handling system uses severity levels to implement sophisticated
/// retry and recovery strategies. Different severity levels trigger different behaviors
/// during task queue processing.
///
/// ## Severity Levels
///
/// ### Temporary
/// Indicates transient issues that should be retried automatically:
/// - **Network timeouts**: L1/L2 RPC connection issues
/// - **Rate limiting**: Temporary API throttling
/// - **EL unavailability**: Execution layer temporarily unresponsive
/// - **Resource constraints**: Temporary memory/CPU limitations
///
/// **Handling**: Task remains in queue and retries immediately in a loop
///
/// ### Critical  
/// Indicates permanent failures that cannot be resolved by retrying:
/// - **Invalid payloads**: Malformed execution payloads
/// - **Consensus violations**: Operations violating chain rules
/// - **Authentication failures**: Permanent JWT/credential issues
/// - **Configuration errors**: Invalid rollup configuration
///
/// **Handling**: Error propagated to engine actor, task execution halts
///
/// ### Reset
/// Indicates the engine state has become inconsistent and requires reset:
/// - **Fork conflicts**: Irreconcilable forkchoice disagreements  
/// - **State corruption**: Local state inconsistent with L1 data
/// - **Deep reorgs**: L1 reorganization beyond safe confirmation depth
/// - **Sync failures**: Cannot maintain synchronization with L1 chain
///
/// **Handling**: Engine reset triggered to find new sync starting point
///
/// ### Flush
/// Indicates the derivation pipeline needs to be flushed and restarted:
/// - **Invalid span batches**: Sequencer data violates protocol rules
/// - **Channel timeouts**: Batch channels exceed timeout limits  
/// - **Batch validation failures**: Cannot validate sequencer submissions
/// - **Pipeline corruption**: Derivation state becomes inconsistent
///
/// **Handling**: Derivation pipeline flush requested from external systems
#[derive(Debug, PartialEq, Eq, Display, Clone, Copy)]
pub enum EngineTaskErrorSeverity {
    /// The error is temporary and the task is retried.
    #[display("temporary")]
    Temporary,
    /// The error is critical and is propagated to the engine actor.
    #[display("critical")]
    Critical,
    /// The error indicates that the engine should be reset.
    #[display("reset")]
    Reset,
    /// The error indicates that the engine should be flushed.
    #[display("flush")]
    Flush,
}

/// Interface for engine task errors, providing severity classification for error handling.
///
/// All engine task errors must implement this trait to enable the sophisticated
/// error handling and retry logic in the task execution system. The severity
/// level determines how the error is processed during task queue draining.
///
/// ## Implementation Guidelines
/// When implementing this trait:
/// - **Be Conservative**: Prefer higher severity when uncertain
/// - **Consider Context**: Same error type may have different severities in different contexts
/// - **Document Reasoning**: Clearly document why specific severity levels are chosen
///
/// ## Example Implementation
/// ```ignore
/// impl EngineTaskError for MyTaskError {
///     fn severity(&self) -> EngineTaskErrorSeverity {
///         match self {
///             Self::NetworkTimeout => EngineTaskErrorSeverity::Temporary,
///             Self::InvalidPayload => EngineTaskErrorSeverity::Critical,
///             Self::StateCorruption => EngineTaskErrorSeverity::Reset,
///             Self::BatchInvalid => EngineTaskErrorSeverity::Flush,
///         }
///     }
/// }
/// ```
pub trait EngineTaskError {
    /// The severity of the error.
    fn severity(&self) -> EngineTaskErrorSeverity;
}

/// Interface for engine task execution, defining how tasks interact with engine state.
///
/// This trait provides the contract for all engine tasks, ensuring consistent
/// execution patterns and error handling across different task types.
///
/// ## Design Principles
/// - **Atomicity**: Tasks either complete fully or leave state unchanged
/// - **Exclusive Access**: Tasks receive mutable access to engine state during execution
/// - **Error Transparency**: All errors are properly classified by severity
/// - **State Consistency**: Tasks must maintain engine state invariants
///
/// ## Implementation Requirements
/// Task implementations must:
/// 1. **Validate Preconditions**: Check that engine state supports the operation
/// 2. **Execute Atomically**: Apply all changes or none if any step fails
/// 3. **Update State**: Modify engine state to reflect successful completion
/// 4. **Handle Errors**: Return appropriate error severity for different failure modes
///
/// ## Execution Context
/// Tasks execute with:
/// - **Exclusive state access**: No other tasks can run concurrently
/// - **RPC connectivity**: Access to L1/L2 providers through engine client
/// - **Configuration**: Access to rollup configuration for validation
/// - **Metrics**: Automatic instrumentation for performance monitoring
#[async_trait]
pub trait EngineTaskExt {
    /// The output type of the task.
    type Output;

    /// The error type of the task.
    type Error: EngineTaskError;

    /// Executes the task, taking a shared lock on the engine state and `self`.
    async fn execute(&self, state: &mut EngineState) -> Result<Self::Output, Self::Error>;
}

/// Comprehensive error type covering all possible engine task execution failures.
///
/// This enum unifies error handling across all task types, enabling centralized
/// error processing and consistent severity classification. Each variant wraps
/// the specific error type from individual task implementations.
///
/// ## Error Categories
///
/// ### Forkchoice Errors
/// Issues with forkchoice state updates and synchronization:
/// - Invalid forkchoice states rejected by execution layer
/// - Network failures during forkchoice RPC calls
/// - State inconsistencies requiring engine reset
///
/// ### Insert Errors  
/// Problems inserting unsafe blocks from gossip network:
/// - Malformed execution payloads from peers
/// - Blocks that don't extend current unsafe chain
/// - Execution layer rejection of unsafe block data
///
/// ### Build Errors
/// Failures during block building and payload creation:
/// - Invalid payload attributes from sequencer
/// - Execution layer cannot build requested block
/// - Timeout waiting for payload construction
///
/// ### Consolidate Errors
/// Issues during safe chain consolidation via L1 derivation:
/// - Derived attributes don't match unsafe block
/// - L1 data unavailable for derivation process
/// - Span batch validation failures
///
/// ### Finalize Errors  
/// Problems finalizing L2 blocks based on L1 finality:
/// - Finalized L1 blocks not available
/// - L2 finalization conflicts with unsafe chain
/// - System config updates during finalization
///
/// ## Error Severity Inheritance
/// The severity of this error matches the severity of the wrapped error,
/// enabling consistent error handling regardless of which task type failed.
#[derive(Error, Debug)]
pub enum EngineTaskErrors {
    /// An error that occurred while updating the forkchoice state.
    #[error(transparent)]
    Forkchoice(#[from] ForkchoiceTaskError),
    /// An error that occurred while inserting a block into the engine.
    #[error(transparent)]
    Insert(#[from] InsertTaskError),
    /// An error that occurred while building a block.
    #[error(transparent)]
    Build(#[from] BuildTaskError),
    /// An error that occurred while consolidating the engine state.
    #[error(transparent)]
    Consolidate(#[from] ConsolidateTaskError),
    /// An error that occurred while finalizing an L2 block.
    #[error(transparent)]
    Finalize(#[from] FinalizeTaskError),
}

impl EngineTaskError for EngineTaskErrors {
    fn severity(&self) -> EngineTaskErrorSeverity {
        match self {
            Self::Forkchoice(inner) => inner.severity(),
            Self::Insert(inner) => inner.severity(),
            Self::Build(inner) => inner.severity(),
            Self::Consolidate(inner) => inner.severity(),
            Self::Finalize(inner) => inner.severity(),
        }
    }
}

/// Enumeration of all possible engine tasks, representing different blockchain operations.
///
/// Each task type corresponds to a specific operation in the OP Stack rollup node:
/// - **State Synchronization**: Maintaining consistency with L1 and other L2 nodes
/// - **Block Processing**: Building, inserting, and validating L2 blocks  
/// - **Chain Management**: Advancing safe/finalized heads based on L1 data
///
/// ## Task Priority System
///
/// Tasks are ordered by priority using the [`Ord`] implementation:
///
/// 1. **ForkchoiceUpdate** (Highest Priority)
///    - **Purpose**: Synchronize engine's view of canonical chain
///    - **Urgency**: Must process immediately to maintain consistency
///    - **Frequency**: Triggered by L1 finality updates and unsafe head changes
///    - **Dependencies**: None (can always execute)
///
/// 2. **Build** (High Priority)  
///    - **Purpose**: Create new blocks with sequencer-provided attributes
///    - **Urgency**: Sequencer operations need low latency for user experience
///    - **Frequency**: Every L2 block creation (every 2 seconds on OP Mainnet)
///    - **Dependencies**: Requires current unsafe head as parent
///
/// 3. **Insert** (Medium Priority)
///    - **Purpose**: Import unsafe blocks gossiped from other nodes
///    - **Urgency**: Process gossip promptly to maintain network sync
///    - **Frequency**: When receiving blocks from P2P network
///    - **Dependencies**: Requires known parent block
///
/// 4. **Consolidate** (Low Priority)
///    - **Purpose**: Advance safe chain through L1-derived block validation
///    - **Urgency**: Less time-sensitive, focuses on eventual consistency
///    - **Frequency**: When unsafe head advances beyond safe head
///    - **Dependencies**: Requires L1 derivation data
///
/// 5. **Finalize** (Lowest Priority)
///    - **Purpose**: Mark L2 blocks as finalized based on L1 finality
///    - **Urgency**: Can be delayed without affecting normal operation
///    - **Frequency**: When L1 blocks become finalized (every ~12 minutes)
///    - **Dependencies**: Requires L1 finality information
///
/// ## Task Lifecycle
///
/// 1. **Creation**: External actors create tasks with specific parameters
/// 2. **Enqueuing**: Tasks added to priority queue via [`Engine::enqueue`]
/// 3. **Execution**: Tasks processed in priority order by [`Engine::drain`]
/// 4. **Completion**: Successful tasks update engine state and are removed
/// 5. **Retry**: Failed tasks with temporary errors remain queued for retry
///
/// ## Concurrency Model
///
/// - **Sequential Execution**: Only one task executes at a time
/// - **Atomic Operations**: Each task completes fully or not at all
/// - **State Isolation**: Tasks cannot interfere with each other's execution
/// - **Priority Preemption**: Higher priority tasks always execute first
#[derive(Debug, Clone)]
pub enum EngineTask {
    /// Performs an `engine_forkchoiceUpdated` call to synchronize the execution layer.
    ///
    /// This task updates the execution layer's view of the canonical chain by:
    /// - Setting head, safe, and finalized block hashes from current engine state
    /// - Optionally providing payload attributes for block building
    /// - Handling execution layer responses and updating sync status
    ///
    /// **Contexts for Use**:
    /// - Engine startup to establish initial forkchoice state
    /// - After unsafe head updates from block insertion or building
    /// - During engine reset to establish new canonical chain
    /// - When safe/finalized heads advance through L1 derivation
    ForkchoiceUpdate(ForkchoiceTask),
    
    /// Inserts an execution payload directly into the execution engine.
    ///
    /// This task imports pre-built payloads (typically from gossip network) by:
    /// - Validating payload structure and parent relationships
    /// - Calling `engine_newPayload` to import into execution layer
    /// - Updating engine state if payload represents new unsafe head
    /// - Triggering forkchoice update if payload becomes canonical
    ///
    /// **Contexts for Use**:
    /// - Processing unsafe blocks received from P2P gossip
    /// - Importing blocks from sequencer batch data
    /// - Syncing historical blocks during initial sync
    /// - Processing reorganization blocks during fork resolution
    Insert(InsertTask),
    
    /// Builds a new block with given attributes and imports it into the execution engine.
    ///
    /// This task handles the complete block building process by:
    /// - Requesting forkchoice update with payload attributes
    /// - Waiting for execution layer to build block payload
    /// - Retrieving built payload via `engine_getPayload`
    /// - Importing payload and updating unsafe head state
    ///
    /// **Contexts for Use**:
    /// - Sequencer block production every L2 block interval
    /// - Building blocks for testing and development
    /// - Re-building blocks after rollback/reorg events
    /// - Creating blocks during catch-up sync scenarios
    Build(BuildTask),
    
    /// Performs consolidation to advance the safe chain via L1 derivation.
    ///
    /// This task validates unsafe blocks against L1-derived data by:
    /// - Deriving expected payload attributes from L1 batch data
    /// - Comparing derived attributes with actual unsafe block
    /// - Advancing safe head if attributes match (consolidation success)
    /// - Falling back to block building if attributes don't match
    ///
    /// **Contexts for Use**:
    /// - Advancing safe chain after receiving L1 batch data
    /// - Validating sequencer blocks against L1 commitments
    /// - Recovering from invalid unsafe blocks via re-derivation
    /// - Maintaining eventual consistency with L1 state
    Consolidate(ConsolidateTask),
    
    /// Finalizes an L2 block based on L1 finality information.
    ///
    /// This task updates the finalized head when:
    /// - L1 blocks containing L2 batch data become finalized
    /// - Safe L2 blocks can be promoted to finalized status
    /// - Finalization updates need to be communicated to execution layer
    /// - System configuration updates take effect at finalization
    ///
    /// **Contexts for Use**:
    /// - L1 block finalization events (every ~12 minutes)
    /// - Promoting safe blocks to finalized status
    /// - Updating system configuration at finalization boundaries
    /// - Maintaining finality consistency with L1 chain
    Finalize(FinalizeTask),
}

impl EngineTask {
    /// Executes the task without consuming it.
    async fn execute_inner(&self, state: &mut EngineState) -> Result<(), EngineTaskErrors> {
        match self.clone() {
            Self::ForkchoiceUpdate(task) => task.execute(state).await.map(|_| ())?,
            Self::Insert(task) => task.execute(state).await?,
            Self::Build(task) => task.execute(state).await?,
            Self::Consolidate(task) => task.execute(state).await?,
            Self::Finalize(task) => task.execute(state).await?,
        };

        Ok(())
    }

    const fn task_metrics_label(&self) -> &'static str {
        match self {
            Self::Insert(_) => crate::Metrics::INSERT_TASK_LABEL,
            Self::Consolidate(_) => crate::Metrics::CONSOLIDATE_TASK_LABEL,
            Self::Build(_) => crate::Metrics::BUILD_TASK_LABEL,
            Self::ForkchoiceUpdate(_) => crate::Metrics::FORKCHOICE_TASK_LABEL,
            Self::Finalize(_) => crate::Metrics::FINALIZE_TASK_LABEL,
        }
    }
}

impl PartialEq for EngineTask {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::ForkchoiceUpdate(_), Self::ForkchoiceUpdate(_)) |
                (Self::Insert(_), Self::Insert(_)) |
                (Self::Build(_), Self::Build(_)) |
                (Self::Consolidate(_), Self::Consolidate(_)) |
                (Self::Finalize(_), Self::Finalize(_))
        )
    }
}

impl Eq for EngineTask {}

impl PartialOrd for EngineTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EngineTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Order (descending): ForkchoiceUpdate -> BuildBlock -> InsertUnsafe -> Consolidate
        //
        // https://specs.optimism.io/protocol/derivation.html#forkchoice-synchronization
        //
        // - Outstanding FCUs are processed before anything else.
        // - Block building jobs are prioritized above InsertUnsafe and Consolidate tasks, to give
        //   priority to the sequencer.
        // - InsertUnsafe tasks are prioritized over Consolidate tasks, to ensure that unsafe block
        //   gossip is imported promptly.
        // - Consolidate tasks are the lowest priority, as they are only used for advancing the safe
        //   chain via derivation.
        match (self, other) {
            // Same variant cases
            (Self::Insert(_), Self::Insert(_)) => Ordering::Equal,
            (Self::Consolidate(_), Self::Consolidate(_)) => Ordering::Equal,
            (Self::Build(_), Self::Build(_)) => Ordering::Equal,
            (Self::ForkchoiceUpdate(_), Self::ForkchoiceUpdate(_)) => Ordering::Equal,
            (Self::Finalize(_), Self::Finalize(_)) => Ordering::Equal,

            // Individual ForkchoiceUpdate tasks are the highest priority
            (Self::ForkchoiceUpdate(_), _) => Ordering::Greater,
            (_, Self::ForkchoiceUpdate(_)) => Ordering::Less,

            // BuildBlock tasks are prioritized over InsertUnsafe and Consolidate tasks
            (Self::Build(_), _) => Ordering::Greater,
            (_, Self::Build(_)) => Ordering::Less,

            // InsertUnsafe tasks are prioritized over Consolidate and Finalize tasks
            (Self::Insert(_), _) => Ordering::Greater,
            (_, Self::Insert(_)) => Ordering::Less,

            // Consolidate tasks are prioritized over Finalize tasks
            (Self::Consolidate(_), _) => Ordering::Greater,
            (_, Self::Consolidate(_)) => Ordering::Less,
        }
    }
}

#[async_trait]
impl EngineTaskExt for EngineTask {
    type Output = ();

    type Error = EngineTaskErrors;

    /// Executes the engine task with sophisticated retry logic and error handling.
    ///
    /// This method implements the core task execution pattern used throughout the engine:
    ///
    /// ## Execution Flow
    /// 1. **Retry Loop**: Continuously attempt task execution until success or critical failure
    /// 2. **Error Classification**: Analyze failure severity to determine appropriate response
    /// 3. **Metrics Recording**: Track task failures and successes for monitoring
    /// 4. **Logging**: Provide appropriate log levels based on error severity
    ///
    /// ## Error Handling Strategy
    ///
    /// ### Temporary Errors (Automatic Retry)
    /// ```ignore
    /// // Network timeout, rate limiting, temporary EL unavailability
    /// while let Err(e) = task.execute_inner(state).await {
    ///     if e.severity() == EngineTaskErrorSeverity::Temporary {
    ///         trace!(target: "engine", "{e}");
    ///         continue; // Retry immediately
    ///     }
    ///     // Handle other severities...
    /// }
    /// ```
    ///
    /// ### Critical Errors (Immediate Failure)  
    /// ```ignore
    /// // Invalid payloads, consensus violations, auth failures
    /// EngineTaskErrorSeverity::Critical => {
    ///     error!(target: "engine", "{e}");
    ///     return Err(e); // Propagate to caller
    /// }
    /// ```
    ///
    /// ### Reset Errors (Engine Reset Required)
    /// ```ignore
    /// // State corruption, fork conflicts, deep reorgs
    /// EngineTaskErrorSeverity::Reset => {
    ///     warn!(target: "engine", "Engine requested derivation reset");
    ///     return Err(e); // Trigger engine reset
    /// }
    /// ```
    ///
    /// ### Flush Errors (Pipeline Flush Required)
    /// ```ignore
    /// // Invalid span batches, channel timeouts, batch validation failures
    /// EngineTaskErrorSeverity::Flush => {
    ///     warn!(target: "engine", "Engine requested derivation flush");
    ///     return Err(e); // Trigger pipeline flush
    /// }
    /// ```
    ///
    /// ## Retry Characteristics
    /// - **No Backoff**: Retries happen immediately for temporary errors
    /// - **Unlimited Attempts**: No maximum retry count for temporary errors  
    /// - **No Rate Limiting**: Relies on underlying systems for rate control
    /// - **Context Preservation**: Error context maintained across retries
    ///
    /// ## Metrics Integration
    /// Each execution path records metrics:
    /// - **Failure Metrics**: Categorized by task type and error severity
    /// - **Success Metrics**: Track successful task completions by type
    /// - **Duration Tracking**: Execution time measurement (in underlying methods)
    ///
    /// ## Thread Safety
    /// Task execution is inherently thread-safe because:
    /// - Only one task executes at a time (sequential processing)
    /// - Tasks receive exclusive mutable access to engine state
    /// - No shared mutable state between concurrent task executions
    async fn execute(&self, state: &mut EngineState) -> Result<(), Self::Error> {
        // Retry the task until it succeeds or a critical error occurs.
        while let Err(e) = self.execute_inner(state).await {
            let severity = e.severity();

            kona_macros::inc!(
                counter,
                crate::Metrics::ENGINE_TASK_FAILURE,
                self.task_metrics_label() => severity.to_string()
            );

            match severity {
                EngineTaskErrorSeverity::Temporary => {
                    trace!(target: "engine", "{e}");
                    continue;
                }
                EngineTaskErrorSeverity::Critical => {
                    error!(target: "engine", "{e}");
                    return Err(e);
                }
                EngineTaskErrorSeverity::Reset => {
                    warn!(target: "engine", "Engine requested derivation reset");
                    return Err(e);
                }
                EngineTaskErrorSeverity::Flush => {
                    warn!(target: "engine", "Engine requested derivation flush");
                    return Err(e);
                }
            }
        }

        kona_macros::inc!(counter, crate::Metrics::ENGINE_TASK_SUCCESS, self.task_metrics_label());

        Ok(())
    }
}
