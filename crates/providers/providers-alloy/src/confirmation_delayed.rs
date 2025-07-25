//! Confirmation-delayed provider implementation.

use alloy_eips::BlockNumberOrTag;
use alloy_provider::{Provider, RootProvider};
use alloy_transport::{RpcError, TransportErrorKind};

/// A wrapper around [`RootProvider`] that delays the view of the chain by a configurable number
/// of blocks to protect against L1 reorgs.
///
/// Any block-by-number call will hide `confirmation_depth` blocks inclusive relative to the
/// actual tip of the L1 chain.
#[derive(Debug, Clone)]
pub struct ConfirmationDelayedProvider {
    /// The inner provider.
    inner: RootProvider,
    /// Number of blocks to hide from the tip of the chain.
    confirmation_depth: u64,
}

/// Error type for confirmation delay operations.
#[derive(Debug, thiserror::Error)]
#[error("Block number {block_number} is within confirmation depth {confirmation_depth}")]
pub struct ConfirmationDelayError {
    /// The block number that was requested.
    pub block_number: u64,
    /// The configured confirmation depth.
    pub confirmation_depth: u64,
}

impl ConfirmationDelayedProvider {
    /// Creates a new [`ConfirmationDelayedProvider`] with the given inner provider and
    /// confirmation depth.
    pub fn new(inner: RootProvider, confirmation_depth: u64) -> Self {
        Self { inner, confirmation_depth }
    }

    /// Creates a new [`ConfirmationDelayedProvider`] from the provided [reqwest::Url] and
    /// confirmation depth.
    pub fn new_http(url: reqwest::Url, confirmation_depth: u64) -> Self {
        let inner = RootProvider::new_http(url);
        Self::new(inner, confirmation_depth)
    }

    /// Returns the delayed block number by subtracting the confirmation depth from the given
    /// block number, returning None if this would result in an underflow.
    pub fn apply_confirmation_delay(&self, block_number: u64) -> Option<u64> {
        block_number.checked_sub(self.confirmation_depth)
    }

    /// Converts a block number request to apply confirmation delay, returning the delayed block
    /// number or an error if the delay would result in requesting a negative block.
    pub async fn delay_block_number(
        &self,
        number: BlockNumberOrTag,
    ) -> Result<BlockNumberOrTag, RpcError<TransportErrorKind>> {
        match number {
            BlockNumberOrTag::Number(n) => {
                // Apply confirmation delay to specific block numbers
                self.apply_confirmation_delay(n)
                    .map(BlockNumberOrTag::Number)
                    .ok_or_else(|| {
                        RpcError::Transport(TransportErrorKind::Custom(Box::new(
                            ConfirmationDelayError {
                                block_number: n,
                                confirmation_depth: self.confirmation_depth,
                            }
                        )))
                    })
            }
            BlockNumberOrTag::Latest => {
                // For "latest", get the actual latest block and apply delay
                let latest = self.inner.get_block_number().await?;
                self.apply_confirmation_delay(latest)
                    .map(BlockNumberOrTag::Number)
                    .ok_or_else(|| {
                        RpcError::Transport(TransportErrorKind::Custom(Box::new(
                            ConfirmationDelayError {
                                block_number: latest,
                                confirmation_depth: self.confirmation_depth,
                            }
                        )))
                    })
            }
            // Pass through other tags unchanged
            other => Ok(other),
        }
    }

    /// Returns the inner provider for methods that don't need confirmation delay.
    /// The caller is responsible for applying delay where appropriate.
    pub fn inner(&self) -> &RootProvider {
        &self.inner
    }

    /// Get delayed block number.
    pub async fn get_block_number(&self) -> Result<u64, RpcError<TransportErrorKind>> {
        let latest = self.inner.get_block_number().await?;
        self.apply_confirmation_delay(latest).ok_or_else(|| {
            RpcError::Transport(TransportErrorKind::Custom(Box::new(
                ConfirmationDelayError {
                    block_number: latest,
                    confirmation_depth: self.confirmation_depth,
                }
            )))
        })
    }

    /// Get chain ID (no delay needed).
    pub async fn get_chain_id(&self) -> Result<u64, RpcError<TransportErrorKind>> {
        // Chain ID is static, no delay needed
        self.inner.get_chain_id().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_confirmation_delay() {
        let provider = ConfirmationDelayedProvider::new(
            RootProvider::new_http("http://localhost:8545".parse().unwrap()),
            4,
        );

        // Test normal case
        assert_eq!(provider.apply_confirmation_delay(10), Some(6));

        // Test edge case
        assert_eq!(provider.apply_confirmation_delay(4), Some(0));

        // Test underflow
        assert_eq!(provider.apply_confirmation_delay(3), None);
        assert_eq!(provider.apply_confirmation_delay(0), None);
    }

    #[tokio::test]
    async fn test_delay_block_number() {
        let provider = ConfirmationDelayedProvider::new(
            RootProvider::new_http("http://localhost:8545".parse().unwrap()),
            4,
        );

        // Test specific block number
        let delayed = provider
            .delay_block_number(BlockNumberOrTag::Number(10))
            .await
            .unwrap();
        assert_eq!(delayed, BlockNumberOrTag::Number(6));

        // Test underflow case
        let result = provider
            .delay_block_number(BlockNumberOrTag::Number(2))
            .await;
        assert!(result.is_err());
    }
}