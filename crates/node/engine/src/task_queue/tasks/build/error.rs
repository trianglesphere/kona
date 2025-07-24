//! Contains error types for the [crate::ForkchoiceTask].

use crate::{
    EngineTaskError, ForkchoiceTaskError, InsertTaskError,
    task_queue::tasks::task::EngineTaskErrorSeverity,
};
use alloy_rpc_types_engine::PayloadStatusEnum;
use alloy_transport::{RpcError, TransportErrorKind};
use kona_protocol::FromBlockError;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use thiserror::Error;
use tokio::sync::mpsc;

/// Errors that can occur during [`BuildTask`] execution with detailed context and recovery guidance.
///
/// Block building is a complex multi-step process involving forkchoice updates, payload
/// construction, retrieval, and import. Each step can fail for different reasons, requiring
/// appropriate error classification and recovery strategies.
///
/// ## Error Categories by Build Phase
///
/// ### Initial Setup Errors (Temporary)
///
/// #### No Forkchoice Update Needed
/// - **Phase**: Initial forkchoice update with attributes
/// - **Context**: Engine state already matches requested forkchoice
/// - **Recovery**: Skip redundant update, proceed with build request
///
/// #### Engine Syncing  
/// - **Phase**: Any Engine API call during build process
/// - **Context**: Execution layer still performing initial synchronization
/// - **Recovery**: Wait for EL sync completion, retry build operation
///
/// ### Payload Construction Errors
///
/// #### Missing Payload ID (Critical)
/// - **Phase**: After initial forkchoice update
/// - **Context**: EL should return payload ID but returned None
/// - **Cause**: EL implementation bug, invalid attributes, resource exhaustion
/// - **Recovery**: Check EL logs, validate attributes, review resource limits
///
/// #### Get Payload Failed (Temporary)
/// - **Phase**: Retrieving built payload from EL
/// - **Context**: RPC call to `engine_getPayload` failed
/// - **Cause**: Network issues, EL busy, payload not ready
/// - **Recovery**: Retry with backoff, check EL status
///
/// ### Payload Import Errors
///
/// #### New Payload Failed (Temporary)
/// - **Phase**: Importing built payload via `engine_newPayload`
/// - **Context**: RPC call failed during payload import
/// - **Cause**: Network issues, EL overloaded, temporary resource constraints
/// - **Recovery**: Retry import operation, monitor EL performance
///
/// #### Unexpected Payload Status (Critical)
/// - **Phase**: Processing `engine_newPayload` response
/// - **Context**: EL returned unexpected status (e.g., INVALID when VALID expected)
/// - **Cause**: Payload construction bug, consensus violation, EL implementation issue
/// - **Recovery**: Review payload construction logic, check EL compatibility
///
/// ### Deposit-Only Payload Handling
///
/// #### Deposit-Only Payload Failed (Critical)
/// - **Phase**: Processing deposits without user transactions
/// - **Context**: Even simplified deposit-only payload cannot be imported
/// - **Cause**: Fundamental issue with deposit processing, L1 data corruption
/// - **Recovery**: Check L1 derivation data, validate deposit transactions
///
/// #### Deposit-Only Payload Reattempt Failed (Critical)
/// - **Phase**: Retry logic for deposit-only payload import
/// - **Context**: Fallback strategy also failed after initial failure
/// - **Cause**: Persistent issue with deposit data or EL state
/// - **Recovery**: Engine reset may be required, check L1 batch data integrity
///
/// ### Protocol-Specific Errors
///
/// #### Holocene Invalid Flush (Flush)
/// - **Phase**: Post-Holocene fork payload validation
/// - **Context**: Payload violates post-Holocene consensus rules
/// - **Cause**: Invalid sequencer batch data, protocol upgrade issues
/// - **Recovery**: Flush derivation pipeline, re-derive from L1 data
///
/// ### Data Conversion Errors
///
/// #### From Block Conversion (Critical)
/// - **Phase**: Converting payload to L2BlockInfo
/// - **Context**: Built payload cannot be parsed as valid L2 block
/// - **Cause**: Malformed payload data, missing OP Stack fields
/// - **Recovery**: Review payload construction, check EL OP Stack compatibility
///
/// #### MPSC Send Error (Critical)
/// - **Phase**: Sending completed payload to result channel
/// - **Context**: Receiver channel closed or full
/// - **Cause**: Consumer stopped processing, memory pressure
/// - **Recovery**: Check consumer health, review channel buffer sizes
///
/// ## Error Severity Guidelines
///
/// - **Temporary**: Network/resource issues that resolve with retry
/// - **Critical**: Logic errors or permanent failures requiring attention
/// - **Flush**: Protocol violations requiring derivation pipeline restart
/// - **Inherited**: Errors from sub-tasks maintain their original severity
#[derive(Debug, Error)]
pub enum BuildTaskError {
    /// The forkchoice update is not needed.
    #[error("No forkchoice update needed")]
    NoForkchoiceUpdateNeeded,
    /// The engine is syncing.
    #[error("Attempting to update forkchoice state while EL syncing")]
    EngineSyncing,
    /// Missing payload ID.
    #[error("Missing payload ID")]
    MissingPayloadId,
    /// The initial forkchoice update call to the engine api failed.
    #[error(transparent)]
    ForkchoiceUpdateFailed(#[from] ForkchoiceTaskError),
    /// Impossible to insert the payload into the engine.
    #[error(transparent)]
    PayloadInsertionFailed(#[from] InsertTaskError),
    /// Unexpected payload status
    #[error("Unexpected payload status: {0}")]
    UnexpectedPayloadStatus(PayloadStatusEnum),
    /// The get payload call to the engine api failed.
    #[error(transparent)]
    GetPayloadFailed(RpcError<TransportErrorKind>),
    /// The new payload call to the engine api failed.
    #[error(transparent)]
    NewPayloadFailed(RpcError<TransportErrorKind>),
    /// A deposit-only payload failed to import.
    #[error("Deposit-only payload failed to import")]
    DepositOnlyPayloadFailed,
    /// Failed to re-atttempt payload import with deposit-only payload.
    #[error("Failed to re-attempt payload import with deposit-only payload")]
    DepositOnlyPayloadReattemptFailed,
    /// The payload is invalid, and the derivation pipeline must
    /// be flushed post-holocene.
    #[error("Invalid payload, must flush post-holocene")]
    HoloceneInvalidFlush,
    /// Failed to convert a [`OpExecutionPayload`] to a [`L2BlockInfo`].
    ///
    /// [`OpExecutionPayload`]: op_alloy_rpc_types_engine::OpExecutionPayload
    /// [`L2BlockInfo`]: kona_protocol::L2BlockInfo
    #[error(transparent)]
    FromBlock(#[from] FromBlockError),
    /// Error sending the built payload envelope.
    #[error(transparent)]
    MpscSend(#[from] mpsc::error::SendError<OpExecutionPayloadEnvelope>),
}

impl EngineTaskError for BuildTaskError {
    fn severity(&self) -> EngineTaskErrorSeverity {
        match self {
            Self::ForkchoiceUpdateFailed(inner) => inner.severity(),
            Self::PayloadInsertionFailed(inner) => inner.severity(),
            Self::NoForkchoiceUpdateNeeded => EngineTaskErrorSeverity::Temporary,
            Self::EngineSyncing => EngineTaskErrorSeverity::Temporary,
            Self::GetPayloadFailed(_) => EngineTaskErrorSeverity::Temporary,
            Self::NewPayloadFailed(_) => EngineTaskErrorSeverity::Temporary,
            Self::HoloceneInvalidFlush => EngineTaskErrorSeverity::Flush,
            Self::MissingPayloadId => EngineTaskErrorSeverity::Critical,
            Self::UnexpectedPayloadStatus(_) => EngineTaskErrorSeverity::Critical,
            Self::DepositOnlyPayloadReattemptFailed => EngineTaskErrorSeverity::Critical,
            Self::DepositOnlyPayloadFailed => EngineTaskErrorSeverity::Critical,
            Self::FromBlock(_) => EngineTaskErrorSeverity::Critical,
            Self::MpscSend(_) => EngineTaskErrorSeverity::Critical,
        }
    }
}
