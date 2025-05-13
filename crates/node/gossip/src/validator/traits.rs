//! Contains the validation trait for handlers.

use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;

/// Validator
///
/// A validator is responsible for validating incoming unsafe blocks
/// in the form of an [`OpNetworkPayloadEnvelope`].
pub trait Validator: Send {
    /// The error type for block validation.
    type Error: Into<libp2p::gossipsub::MessageAcceptance> + std::fmt::Debug;

    /// Validates the given [`OpNetworkPayloadEnvelope`].
    ///
    /// Returns `Ok(())` if the block is valid, or an error if it is not.
    fn validate(&mut self, envelope: &OpNetworkPayloadEnvelope) -> Result<(), Self::Error>;
}
