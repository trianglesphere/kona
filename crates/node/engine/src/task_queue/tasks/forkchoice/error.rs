//! Contains error types for the [crate::ForkchoiceTask].

use crate::{EngineTaskError, task_queue::tasks::task::EngineTaskErrorSeverity};
use alloy_rpc_types_engine::PayloadStatusEnum;
use alloy_transport::{RpcError, TransportErrorKind};
use thiserror::Error;

/// Errors that can occur during [`ForkchoiceTask`] execution.
///
/// Forkchoice update operations can fail for various reasons, ranging from temporary
/// network issues to fundamental inconsistencies in blockchain state. This error type
/// provides detailed context for each failure mode and appropriate severity classification.
///
/// ## Error Categories and Contexts
///
/// ### Temporary Errors (Will Retry)
///
/// #### No Forkchoice Update Needed
/// - **Context**: Engine state already matches requested forkchoice
/// - **Cause**: Redundant forkchoice update request
/// - **Recovery**: Skip operation, continue with next task
///
/// #### Engine Syncing
/// - **Context**: Execution layer is still performing initial sync
/// - **Cause**: EL not yet ready to accept forkchoice updates
/// - **Recovery**: Wait for sync completion, retry periodically
///
/// #### Forkchoice Update Failed (RPC)
/// - **Context**: Network or transport-level failure during RPC call
/// - **Cause**: Connection timeout, JWT auth failure, EL unavailable
/// - **Recovery**: Retry with exponential backoff, check network connectivity
///
/// ### Critical Errors (Immediate Failure)
///
/// #### Finalized Ahead of Unsafe
/// - **Context**: Blockchain state invariant violation detected
/// - **Cause**: Incorrect head ordering (finalized > unsafe)
/// - **Recovery**: Review state management logic, check L1 derivation
///
/// #### Unexpected Payload Status
/// - **Context**: EL returned unknown or inappropriate status
/// - **Cause**: Protocol version mismatch, EL implementation bug
/// - **Recovery**: Check Engine API compatibility, update EL version
///
/// ### Reset Errors (Engine Reset Required)
///
/// #### Invalid Forkchoice State
/// - **Context**: Forkchoice state is fundamentally inconsistent
/// - **Cause**: Missing parent blocks, conflicting chain state
/// - **Recovery**: Engine reset to find new sync starting point
///
/// #### Invalid Payload Status
/// - **Context**: EL rejected forkchoice with INVALID status
/// - **Cause**: Block references invalid execution state
/// - **Recovery**: Engine reset, possible L1 reorg handling
///
/// ## Error Severity Rationale
///
/// The severity levels are chosen based on:
/// - **Temporary**: Issues that resolve automatically over time
/// - **Critical**: Logic errors requiring immediate attention but no reset
/// - **Reset**: State corruption requiring engine state reconstruction
///
/// This classification ensures appropriate error handling and recovery strategies
/// while maintaining system stability and data consistency.
#[derive(Debug, Error)]
pub enum ForkchoiceTaskError {
    /// The forkchoice update is not needed.
    #[error("No forkchoice update needed")]
    NoForkchoiceUpdateNeeded,
    /// The engine is syncing.
    #[error("Attempting to update forkchoice state while EL syncing")]
    EngineSyncing,
    /// The forkchoice update call to the engine api failed.
    #[error("Forkchoice update engine api call failed")]
    ForkchoiceUpdateFailed(RpcError<TransportErrorKind>),
    /// The finalized head is behind the unsafe head.
    #[error("Invalid forkchoice state: unsafe head {0} is ahead of finalized head {1}")]
    FinalizedAheadOfUnsafe(u64, u64),
    /// The forkchoice state is invalid.
    #[error("Invalid forkchoice state")]
    InvalidForkchoiceState,
    /// The payload status is invalid.
    #[error("Invalid payload status: {0}")]
    InvalidPayloadStatus(String),
    /// The payload status is unexpected.
    #[error("Unexpected payload status: {0}")]
    UnexpectedPayloadStatus(PayloadStatusEnum),
}

impl EngineTaskError for ForkchoiceTaskError {
    fn severity(&self) -> EngineTaskErrorSeverity {
        match self {
            Self::NoForkchoiceUpdateNeeded => EngineTaskErrorSeverity::Temporary,
            Self::EngineSyncing => EngineTaskErrorSeverity::Temporary,
            Self::ForkchoiceUpdateFailed(_) => EngineTaskErrorSeverity::Temporary,
            Self::FinalizedAheadOfUnsafe(_, _) => EngineTaskErrorSeverity::Critical,
            Self::UnexpectedPayloadStatus(_) => EngineTaskErrorSeverity::Critical,
            Self::InvalidForkchoiceState => EngineTaskErrorSeverity::Reset,
            Self::InvalidPayloadStatus(_) => EngineTaskErrorSeverity::Reset,
        }
    }
}
