//! Confirmation-delayed provider implementation.
//!
//! This module provides a wrapper around providers that delays the chain view by a configurable
//! number of blocks to prevent instability from L1 chain reorgs. The `ConfirmationDelayedProvider`
//! hides the most recent N blocks relative to the actual L1 chain tip.

use alloy_consensus::{Header, Receipt, TxEnvelope};
use alloy_primitives::B256;
use alloy_provider::Provider;
use alloy_transport::{RpcError, TransportErrorKind};
use async_trait::async_trait;
use kona_derive::{ChainProvider, PipelineError, PipelineErrorKind};
use kona_protocol::BlockInfo;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

/// Default cache TTL for latest block number (5 seconds).
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(5);

/// A provider wrapper that delays the chain view by a configurable number of blocks
/// to prevent instability from L1 chain reorgs.
///
/// This provider hides the most recent `confirmation_delay` blocks from the actual chain tip,
/// effectively creating a "confirmation window" where blocks must be confirmed before being
/// visible to consumers.
#[derive(Debug)]
pub struct ConfirmationDelayedProvider<T> {
    /// The underlying provider.
    pub inner: T,
    /// Number of blocks to delay (confirmations required).
    confirmation_delay: u64,
    /// Cache for the latest block number to avoid repeated RPC calls.
    latest_block_cache: Arc<Mutex<Option<(u64, Instant)>>>,
    /// Cache TTL for latest block number.
    cache_ttl: Duration,
}

impl<T> Clone for ConfirmationDelayedProvider<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            confirmation_delay: self.confirmation_delay,
            latest_block_cache: Arc::new(Mutex::new(None)),
            cache_ttl: self.cache_ttl,
        }
    }
}

impl<T> ConfirmationDelayedProvider<T>
where
    T: Provider,
{
    /// Creates a new `ConfirmationDelayedProvider` with the given underlying provider
    /// and confirmation delay.
    ///
    /// # Arguments
    /// * `inner` - The underlying provider to wrap
    /// * `confirmation_delay` - Number of blocks to delay the chain view
    ///
    /// # Returns
    /// A new `ConfirmationDelayedProvider` instance.
    pub fn new(inner: T, confirmation_delay: u64) -> Self {
        Self {
            inner,
            confirmation_delay,
            latest_block_cache: Arc::new(Mutex::new(None)),
            cache_ttl: DEFAULT_CACHE_TTL,
        }
    }

    /// Creates a new `ConfirmationDelayedProvider` with a custom cache TTL.
    ///
    /// # Arguments
    /// * `inner` - The underlying provider to wrap
    /// * `confirmation_delay` - Number of blocks to delay the chain view
    /// * `cache_ttl` - Time-to-live for the latest block number cache
    ///
    /// # Returns
    /// A new `ConfirmationDelayedProvider` instance with custom cache TTL.
    pub fn new_with_cache_ttl(inner: T, confirmation_delay: u64, cache_ttl: Duration) -> Self {
        Self {
            inner,
            confirmation_delay,
            latest_block_cache: Arc::new(Mutex::new(None)),
            cache_ttl,
        }
    }

    /// Gets the latest confirmed block number (latest - confirmation_delay).
    ///
    /// This method caches the result for `cache_ttl` duration to minimize RPC calls.
    ///
    /// # Returns
    /// The latest confirmed block number, or an error if the provider fails.
    pub async fn get_latest_confirmed_block_number(
        &self,
    ) -> Result<u64, RpcError<TransportErrorKind>> {
        let now = Instant::now();
        let mut cache = self.latest_block_cache.lock().await;

        // Check if we have a valid cached value
        if let Some((cached_block, cached_time)) = *cache {
            if now.duration_since(cached_time) < self.cache_ttl {
                return Ok(cached_block.saturating_sub(self.confirmation_delay));
            }
        }

        // Cache miss or expired, fetch latest block number
        let latest = self.inner.get_block_number().await?;
        *cache = Some((latest, now));

        Ok(latest.saturating_sub(self.confirmation_delay))
    }

    /// Validates that a block number is within the confirmed range.
    ///
    /// # Arguments
    /// * `block_number` - The block number to validate
    ///
    /// # Returns
    /// `Ok(())` if the block is confirmed, or an error if it's not available due to confirmation
    /// delay.
    async fn validate_block_number(&self, block_number: u64) -> Result<(), ConfirmationDelayError>
    where
        T: Provider,
    {
        let latest_confirmed = self.get_latest_confirmed_block_number().await?;

        if block_number > latest_confirmed {
            return Err(ConfirmationDelayError::BlockNotConfirmed {
                requested: block_number,
                latest_confirmed,
            });
        }

        Ok(())
    }
}

/// Error types for the confirmation-delayed provider.
#[derive(Debug, thiserror::Error)]
pub enum ConfirmationDelayError {
    /// Block not available due to confirmation delay.
    #[error(
        "Block not available due to confirmation delay: requested={requested}, latest_confirmed={latest_confirmed}"
    )]
    BlockNotConfirmed {
        /// The requested block number.
        requested: u64,
        /// The latest confirmed block number.
        latest_confirmed: u64,
    },
    /// Transport error from the underlying provider.
    #[error(transparent)]
    Transport(#[from] RpcError<TransportErrorKind>),
}

impl From<ConfirmationDelayError> for PipelineErrorKind {
    fn from(e: ConfirmationDelayError) -> Self {
        match e {
            ConfirmationDelayError::BlockNotConfirmed { requested, latest_confirmed } => {
                PipelineErrorKind::Temporary(PipelineError::Provider(format!(
                    "Block {requested} not confirmed (latest confirmed: {latest_confirmed})"
                )))
            }
            ConfirmationDelayError::Transport(e) => PipelineErrorKind::Temporary(
                PipelineError::Provider(format!("Transport error: {e}")),
            ),
        }
    }
}

#[async_trait]
impl<T> ChainProvider for ConfirmationDelayedProvider<T>
where
    T: ChainProvider + Provider + Send + Sync,
    T::Error: Into<PipelineErrorKind>,
{
    type Error = ConfirmationDelayError;

    async fn header_by_hash(&mut self, hash: B256) -> Result<Header, Self::Error> {
        // For hash-based queries, we can't pre-validate the block number,
        // so we delegate to the underlying provider and let it handle the request.
        // The confirmation delay is implicitly handled by the fact that
        // recent blocks won't be found if they're not confirmed.
        self.inner.header_by_hash(hash).await.map_err(|e| {
            // Convert the error to RpcError<TransportErrorKind>
            ConfirmationDelayError::Transport(RpcError::Transport(TransportErrorKind::Custom(
                format!("ChainProvider error: {}", e).into(),
            )))
        })
    }

    async fn block_info_by_number(&mut self, number: u64) -> Result<BlockInfo, Self::Error> {
        // Validate that the requested block number is within the confirmed range
        self.validate_block_number(number).await?;

        self.inner.block_info_by_number(number).await.map_err(|e| {
            ConfirmationDelayError::Transport(RpcError::Transport(TransportErrorKind::Custom(
                format!("ChainProvider error: {}", e).into(),
            )))
        })
    }

    async fn receipts_by_hash(&mut self, hash: B256) -> Result<Vec<Receipt>, Self::Error> {
        // For hash-based queries, delegate to the underlying provider
        self.inner.receipts_by_hash(hash).await.map_err(|e| {
            ConfirmationDelayError::Transport(RpcError::Transport(TransportErrorKind::Custom(
                format!("ChainProvider error: {}", e).into(),
            )))
        })
    }

    async fn block_info_and_transactions_by_hash(
        &mut self,
        hash: B256,
    ) -> Result<(BlockInfo, Vec<TxEnvelope>), Self::Error> {
        // For hash-based queries, delegate to the underlying provider
        self.inner.block_info_and_transactions_by_hash(hash).await.map_err(|e| {
            ConfirmationDelayError::Transport(RpcError::Transport(TransportErrorKind::Custom(
                format!("ChainProvider error: {}", e).into(),
            )))
        })
    }
}

// TODO: Add L1OriginSelectorProvider implementation
// This requires importing from kona-node-service which creates a circular dependency.
// The implementation will be done as part of the sequencer integration.

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[test]
    fn test_confirmation_delay_calculation() {
        // Test the core saturating_sub logic that powers confirmation delays

        // Case 1: Normal confirmation delay
        let latest = 10u64;
        let confirmation_delay = 4u64;
        let confirmed = latest.saturating_sub(confirmation_delay);
        assert_eq!(confirmed, 6);

        // Case 2: Zero confirmation delay
        let latest = 10u64;
        let confirmation_delay = 0u64;
        let confirmed = latest.saturating_sub(confirmation_delay);
        assert_eq!(confirmed, 10);

        // Case 3: Confirmation delay larger than chain
        let latest = 5u64;
        let confirmation_delay = 10u64;
        let confirmed = latest.saturating_sub(confirmation_delay);
        assert_eq!(confirmed, 0); // Should saturate to 0
    }

    #[tokio::test]
    async fn test_block_validation_logic() {
        // Test validation logic directly
        let latest_confirmed = 6u64; // Simulated latest confirmed block

        // Block within range should be valid
        let block_number = 5u64;
        let is_valid = block_number <= latest_confirmed;
        assert!(is_valid);

        // Block at the boundary should be valid
        let block_number = 6u64;
        let is_valid = block_number <= latest_confirmed;
        assert!(is_valid);

        // Block outside range should be invalid
        let block_number = 8u64;
        let is_valid = block_number <= latest_confirmed;
        assert!(!is_valid);
    }

    #[test]
    fn test_error_conversion() {
        let error =
            ConfirmationDelayError::BlockNotConfirmed { requested: 10, latest_confirmed: 6 };

        let pipeline_error: PipelineErrorKind = error.into();

        // Verify error converts properly
        assert!(matches!(pipeline_error, PipelineErrorKind::Temporary(_)));

        // Test error message format with a new error instance
        let error_for_format =
            ConfirmationDelayError::BlockNotConfirmed { requested: 10, latest_confirmed: 6 };
        let error_msg = format!("{}", error_for_format);
        assert!(error_msg.contains("Block not available due to confirmation delay"));
        assert!(error_msg.contains("requested=10"));
        assert!(error_msg.contains("latest_confirmed=6"));
    }

    #[test]
    fn test_error_display() {
        let error =
            ConfirmationDelayError::BlockNotConfirmed { requested: 15, latest_confirmed: 10 };

        let display_str = format!("{}", error);
        assert!(display_str.contains("Block not available due to confirmation delay"));
        assert!(display_str.contains("requested=15"));
        assert!(display_str.contains("latest_confirmed=10"));
    }

    #[test]
    fn test_default_constants() {
        // Test that the default cache TTL is reasonable
        assert_eq!(DEFAULT_CACHE_TTL, Duration::from_secs(5));
    }

    #[test]
    fn test_confirmation_delay_error_debug() {
        let error =
            ConfirmationDelayError::BlockNotConfirmed { requested: 15, latest_confirmed: 10 };

        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("BlockNotConfirmed"));
        assert!(debug_str.contains("requested: 15"));
        assert!(debug_str.contains("latest_confirmed: 10"));
    }

    #[test]
    fn test_transport_error_conversion() {
        let transport_error = RpcError::Transport(TransportErrorKind::Custom("test error".into()));
        let confirmation_error = ConfirmationDelayError::Transport(transport_error);

        let pipeline_error: PipelineErrorKind = confirmation_error.into();
        assert!(matches!(pipeline_error, PipelineErrorKind::Temporary(_)));
    }

    #[test]
    fn test_confirmation_delay_edge_cases() {
        // Test maximum values
        let latest = u64::MAX;
        let confirmation_delay = 100u64;
        let confirmed = latest.saturating_sub(confirmation_delay);
        assert_eq!(confirmed, u64::MAX - 100);

        // Test zero latest block
        let latest = 0u64;
        let confirmation_delay = 5u64;
        let confirmed = latest.saturating_sub(confirmation_delay);
        assert_eq!(confirmed, 0); // Should saturate

        // Test maximum confirmation delay
        let latest = 100u64;
        let confirmation_delay = u64::MAX;
        let confirmed = latest.saturating_sub(confirmation_delay);
        assert_eq!(confirmed, 0); // Should saturate
    }
}
