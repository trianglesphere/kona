//! Runtime loader error type.

use alloy_eips::BlockId;
use alloy_transport::{RpcError, TransportErrorKind};
use kona_providers_alloy::AlloyChainProviderError;
use kona_rpc::ProtocolVersionError;

/// Error type for the runtime loader.
#[derive(thiserror::Error, Debug)]
pub enum RuntimeLoaderError {
    /// Transport error
    #[error(transparent)]
    Transport(#[from] RpcError<TransportErrorKind>),
    /// Transport error string.
    #[error("Transport error: {0}")]
    TransportString(String),
    /// An error resulting from decoding the protocol version
    #[error("Failed to decode protocol version: {0}")]
    ProtocolVersionDecode(#[from] ProtocolVersionError),
    /// An error occured from the [`kona_providers_alloy::AlloyChainProvider`].
    #[error(transparent)]
    ChainProvider(#[from] kona_providers_alloy::AlloyChainProviderError),
    /// Failed to convert the address slot bytes to an address
    #[error("Failed to convert address slot bytes to address: {0}")]
    AddressConversion(alloy_primitives::U256),
    /// The block was not found
    #[error("Block not found: {0}")]
    BlockNotFound(BlockId),
    /// Failed to convert the RPC receipts into consensus receipts
    #[error("Failed to convert RPC receipts into consensus receipts: {0}")]
    ReceiptsConversion(alloy_primitives::B256),
}

// Custom implementation of the `Clone` trait converting the `AlloyChainProviderError` to
// `RuntimeLoaderError`
impl Clone for RuntimeLoaderError {
    fn clone(&self) -> Self {
        match self {
            Self::Transport(e) => Self::TransportString(e.to_string()),
            Self::TransportString(e) => Self::TransportString(e.clone()),
            Self::ProtocolVersionDecode(e) => Self::ProtocolVersionDecode(*e),
            Self::ChainProvider(AlloyChainProviderError::BlockNotFound(id)) => {
                Self::BlockNotFound(*id)
            }
            Self::ChainProvider(AlloyChainProviderError::ReceiptsConversion(e)) => {
                Self::ReceiptsConversion(*e)
            }
            Self::ChainProvider(AlloyChainProviderError::Transport(e)) => {
                Self::TransportString(e.to_string())
            }
            Self::AddressConversion(e) => Self::AddressConversion(*e),
            Self::BlockNotFound(id) => Self::BlockNotFound(*id),
            Self::ReceiptsConversion(e) => Self::ReceiptsConversion(*e),
        }
    }
}
