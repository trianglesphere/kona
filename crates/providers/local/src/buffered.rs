//! Buffered L2 Provider implementation that wraps an underlying provider with caching and reorg
//! handling.

use crate::buffer::{ChainBufferError, ChainStateBuffer, ChainStateEvent};
use alloy_primitives::B256;
use async_trait::async_trait;
use kona_derive::{L2ChainProvider, PipelineError, PipelineErrorKind};
use kona_genesis::{RollupConfig, SystemConfig};
use kona_protocol::{BatchValidationProvider, L2BlockInfo};
use kona_providers_alloy::{AlloyL2ChainProvider, AlloyL2ChainProviderError};
use op_alloy_consensus::OpBlock;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// A buffered L2 provider that caches chain state and handles reorgs
#[derive(Debug)]
pub struct BufferedL2Provider {
    /// The underlying L2 chain provider
    inner: Arc<RwLock<AlloyL2ChainProvider>>,
    /// Chain state buffer for caching
    buffer: Arc<ChainStateBuffer>,
    /// Current chain head we're tracking
    current_head: RwLock<Option<B256>>,
}

impl BufferedL2Provider {
    /// Create a new buffered L2 provider
    pub fn new(inner: AlloyL2ChainProvider, cache_size: usize, max_reorg_depth: u64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(inner)),
            buffer: Arc::new(ChainStateBuffer::new(cache_size, max_reorg_depth)),
            current_head: RwLock::new(None),
        }
    }

    /// Process a chain state event (called by ExEx notifications)
    pub async fn handle_chain_event(
        &self,
        event: ChainStateEvent,
    ) -> Result<(), BufferedProviderError> {
        debug!("Handling chain event: {:?}", event);

        // Update our tracked head based on the event
        match &event {
            ChainStateEvent::ChainCommitted { new_head, .. } |
            ChainStateEvent::ChainReorged { new_head, .. } |
            ChainStateEvent::ChainReverted { new_head, .. } => {
                let mut current_head = self.current_head.write().await;
                *current_head = Some(*new_head);
            }
        }

        // Handle the event in the buffer
        self.buffer.handle_event(event).await.map_err(BufferedProviderError::Buffer)?;

        Ok(())
    }

    /// Get the current chain head
    pub async fn current_head(&self) -> Option<B256> {
        let current_head = self.current_head.read().await;
        *current_head
    }

    /// Fallback to the underlying provider for L2 block info by number
    async fn fallback_l2_block_info_by_number(
        &self,
        number: u64,
    ) -> Result<L2BlockInfo, BufferedProviderError> {
        let mut inner = self.inner.write().await;
        inner.l2_block_info_by_number(number).await.map_err(BufferedProviderError::Provider)
    }

    /// Fallback to the underlying provider for full block by number
    async fn fallback_block_by_number(
        &self,
        number: u64,
    ) -> Result<OpBlock, BufferedProviderError> {
        let mut inner = self.inner.write().await;
        inner.block_by_number(number).await.map_err(BufferedProviderError::Provider)
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> crate::buffer::CacheStats {
        self.buffer.cache_stats().await
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        self.buffer.clear().await;
    }
}

/// Clone implementation for BufferedL2Provider
impl Clone for BufferedL2Provider {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            buffer: self.buffer.clone(),
            current_head: RwLock::new(None),
        }
    }
}

#[async_trait]
impl L2ChainProvider for BufferedL2Provider {
    type Error = BufferedProviderError;

    async fn system_config_by_number(
        &mut self,
        number: u64,
        rollup_config: Arc<RollupConfig>,
    ) -> Result<SystemConfig, <Self as L2ChainProvider>::Error> {
        debug!(block_number = number, "Fetching system config by number");

        // For now, delegate to the underlying provider
        // TODO: Add caching for system configs if needed
        let mut inner = self.inner.write().await;
        inner
            .system_config_by_number(number, rollup_config)
            .await
            .map_err(BufferedProviderError::Provider)
    }
}

#[async_trait]
impl BatchValidationProvider for BufferedL2Provider {
    type Error = BufferedProviderError;

    async fn block_by_number(&mut self, number: u64) -> Result<OpBlock, Self::Error> {
        debug!(block_number = number, "Fetching full block by number for batch validation");

        // Check cache first - for OpBlock we need to delegate as it's more complex
        // For now, always delegate to underlying provider
        self.fallback_block_by_number(number).await
    }

    async fn l2_block_info_by_number(&mut self, number: u64) -> Result<L2BlockInfo, Self::Error> {
        debug!(block_number = number, "Fetching L2 block info by number");

        // For now, delegate to the underlying provider
        // TODO: Add caching for L2BlockInfo if needed
        self.fallback_l2_block_info_by_number(number).await
    }
}

/// Errors that can occur in the buffered provider
#[derive(Debug, thiserror::Error)]
pub enum BufferedProviderError {
    /// Error from the underlying provider
    #[error("Provider error: {0}")]
    Provider(#[from] AlloyL2ChainProviderError),
    /// Error from the chain buffer
    #[error("Buffer error: {0}")]
    Buffer(#[from] ChainBufferError),
    /// Block conversion error
    #[error("Block conversion error: {0}")]
    BlockConversion(String),
}

impl From<BufferedProviderError> for PipelineErrorKind {
    fn from(e: BufferedProviderError) -> Self {
        match e {
            BufferedProviderError::Provider(provider_err) => provider_err.into(),
            BufferedProviderError::Buffer(ChainBufferError::ReorgTooDeep { depth, max_depth }) => {
                PipelineErrorKind::Critical(PipelineError::Provider(format!(
                    "Reorg too deep: {depth} > {max_depth}"
                )))
            }
            BufferedProviderError::Buffer(ChainBufferError::BlockNotFound { hash }) => {
                PipelineErrorKind::Temporary(PipelineError::Provider(format!(
                    "Block not found in cache: {hash}"
                )))
            }
            BufferedProviderError::BlockConversion(msg) => PipelineErrorKind::Critical(
                PipelineError::Provider(format!("Block conversion error: {msg}")),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use alloy_provider::RootProvider;
    use kona_genesis::RollupConfig;
    use kona_providers_alloy::AlloyL2ChainProvider;
    use op_alloy_network::Optimism;

    async fn create_test_provider() -> BufferedL2Provider {
        // Create a mock alloy provider - in real usage this would connect to a real endpoint
        let url = "http://localhost:8545".parse().unwrap();
        let alloy_provider = RootProvider::<Optimism>::new_http(url);
        let rollup_config = Arc::new(RollupConfig::default());
        let inner = AlloyL2ChainProvider::new(alloy_provider, rollup_config, 100);

        BufferedL2Provider::new(inner, 100, 10)
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let provider = create_test_provider().await;
        assert!(provider.current_head().await.is_none());
    }

    #[tokio::test]
    async fn test_chain_event_handling() {
        let provider = create_test_provider().await;

        let hash1 = B256::with_last_byte(1);
        let hash2 = B256::with_last_byte(2);

        let event =
            ChainStateEvent::ChainCommitted { new_head: hash2, committed: vec![hash1, hash2] };

        let result = provider.handle_chain_event(event).await;
        assert!(result.is_ok());

        assert_eq!(provider.current_head().await, Some(hash2));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let provider = create_test_provider().await;
        let stats = provider.cache_stats().await;

        assert_eq!(stats.capacity, 100);
        assert_eq!(stats.max_reorg_depth, 10);
        assert_eq!(stats.blocks_by_hash_len, 0);
        assert_eq!(stats.blocks_by_number_len, 0);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let provider = create_test_provider().await;

        // Clear should work even on empty cache
        provider.clear_cache().await;

        let stats = provider.cache_stats().await;
        assert_eq!(stats.blocks_by_hash_len, 0);
        assert_eq!(stats.blocks_by_number_len, 0);
    }
}
