//! Error type for block handling.

/// An error occured when encoding the payload from the block handler.
#[derive(Debug, thiserror::Error)]
pub enum HandlerEncodeError {
    /// Failed to encode the payload envelope.
    #[error("Failed to encode payload: {0}")]
    PayloadEncodeError(#[from] op_alloy_rpc_types_engine::PayloadEnvelopeEncodeError),
    /// The topic is unknown.
    #[error("Unknown topic: {0}")]
    UnknownTopic(libp2p::gossipsub::TopicHash),
}
