//! An error type for Block Validation.

use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::PayloadError;
use op_alloy_rpc_types_engine::OpPayloadError;

/// Error that can occur when validating a block.
#[derive(Debug, thiserror::Error)]
pub enum BlockValidationError {
    /// The block has an invalid timestamp.
    #[error("Invalid timestamp. Current: {current}, Received: {received}")]
    Timestamp {
        /// The current timestamp.
        current: u64,
        /// The received timestamp.
        received: u64,
    },
    /// The block has an invalid base fee per gas.
    #[error("Base fee per gas overflow")]
    BaseFeePerGasOverflow(#[from] PayloadError),
    /// The block has an invalid hash.
    #[error("Invalid block hash. Expected: {expected}, Received: {received}")]
    BlockHash {
        /// The expected block hash.
        expected: B256,
        /// The received block hash.
        received: B256,
    },
    /// The block has an invalid signature.
    #[error("Invalid signature.")]
    Signature,
    /// The block has an invalid signer.
    #[error("Invalid signer, expected: {expected}, received: {received}")]
    Signer {
        /// The expected signer.
        expected: Address,
        /// The received signer.
        received: Address,
    },
    /// Invalid block.
    #[error(transparent)]
    InvalidBlock(#[from] OpPayloadError),
    /// The block has an invalid parent beacon block root.
    #[error("Payload is on v3+ topic, but has empty parent beacon root")]
    ParentBeaconRoot,
    /// The block has an invalid blob gas used.
    #[error("Payload is on v3+ topic, but has non-zero blob gas used")]
    BlobGasUsed,
    /// The block has an invalid excess blob gas.
    #[error("Payload is on v3+ topic, but has non-zero excess blob gas")]
    ExcessBlobGas,
    /// The block has an invalid withdrawals root.
    #[error("Payload is on v4+ topic, but has non-empty withdrawals root")]
    WithdrawalsRoot,
    /// Too many blocks were validated for the same height.
    #[error("Too many blocks seen for height {height}")]
    TooManyBlocks {
        /// The height of the block.
        height: u64,
    },
    /// The block has already been seen.
    #[error("Block seen before")]
    BlockSeen {
        /// The hash of the block.
        block_hash: B256,
    },
}

impl From<BlockValidationError> for libp2p::gossipsub::MessageAcceptance {
    fn from(value: BlockValidationError) -> Self {
        // We only want to ignore blocks that we have already seen.
        match value {
            BlockValidationError::BlockSeen { block_hash: _ } => Self::Ignore,
            _ => Self::Reject,
        }
    }
}
