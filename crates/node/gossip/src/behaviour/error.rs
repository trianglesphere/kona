//! Contains the error type returned by the behavior error.

/// An error that can occur when creating a [`crate::Behaviour`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BehaviourError {
    /// The gossipsub behaviour creation failed.
    #[error("gossipsub behaviour creation failed")]
    GossipsubCreationFailed,
    /// Subscription failed.
    #[error("subscription failed")]
    SubscriptionFailed,
}
