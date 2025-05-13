//! Contains the error type for the network driver builder.

/// An error from the [`crate::NetworkBuilder`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkBuilderError {
    /// An error from building the gossip driver.
    #[error(transparent)]
    Gossip(#[from] kona_gossip::BuilderError),
    /// An error from building the discv5 driver.
    #[error(transparent)]
    DiscoveryDriverBuilder(#[from] kona_disc::BuildError),
    /// The unsafe block signer is missing.
    #[error("missing unsafe block signer")]
    UnsafeBlockSignerNotSet,
    /// Missing RPC receiver.
    #[error("missing RPC receiver")]
    MissingRpcReceiver,
    /// Missing the `RollupConfig`.
    #[error("missing RollupConfig")]
    MissingRollupConfig,
}
