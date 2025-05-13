//! Contains gossip driver error types.

use crate::{BehaviourError, HandlerEncodeError};
use derive_more::From;
use thiserror::Error;

/// An error publishing a payload.
#[derive(Debug, Error)]
pub enum PublishError {
    /// An error occurred publishing the payload.
    #[error("Failed to publish payload: {0}")]
    PublishError(#[from] libp2p::gossipsub::PublishError),
    /// An error occured when encoding the payload.
    #[error("Failed to encode payload: {0}")]
    EncodeError(#[from] HandlerEncodeError),
}

/// An error type for the [`crate::Driver`].
#[derive(Debug, Clone, PartialEq, Eq, From, Error)]
pub enum BuilderError {
    /// Missing key pair.
    #[error("missing key pair")]
    MissingKeyPair,
    /// The unsafe block signer is missing.
    #[error("missing unsafe block signer")]
    MissingUnsafeBlockSigner,
    /// A TCP error.
    #[error("TCP error")]
    TcpError,
    /// An error when setting the behaviour on the swarm builder.
    #[error("error setting behaviour on swarm builder")]
    WithBehaviourError,
    /// Missing gossip address.
    #[error("gossip address not set")]
    GossipAddrNotSet,
    /// An error when building the gossip behaviour.
    #[error("error building gossip behaviour")]
    BehaviourError(BehaviourError),
    /// Missing the l2 block time.
    #[error("missing l2 block time")]
    MissingL2BlockTime,
    /// Missing the rollup config.
    #[error("missing rollup config")]
    MissingRollupConfig,
}
