//! Chain state buffer implementation for handling L2 chain events and reorgs.

use alloy_consensus::{Header, Receipt, TxEnvelope};
use alloy_primitives::B256;
use kona_protocol::BlockInfo;
use lru::LruCache;
use std::num::NonZeroUsize;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Events that can affect chain state
#[derive(Debug, Clone)]
pub enum ChainStateEvent {
    /// New blocks have been committed to the canonical chain
    ChainCommitted {
        /// The new chain head
        new_head: B256,
        /// The blocks that were committed
        committed: Vec<B256>,
    },
    /// Chain reorganization occurred
    ChainReorged {
        /// The old chain head before reorg
        old_head: B256,
        /// The new chain head after reorg
        new_head: B256,
        /// The depth of the reorg (how many blocks were reverted)
        depth: u64,
    },
    /// Chain was reverted to a previous state
    ChainReverted {
        /// The old chain head before revert
        old_head: B256,
        /// The new chain head after revert
        new_head: B256,
        /// The blocks that were reverted
        reverted: Vec<B256>,
    },
}

/// Cached block data
#[derive(Debug, Clone)]
pub struct CachedBlock {
    /// Block header
    pub header: Header,
    /// Block receipts
    pub receipts: Option<Vec<Receipt>>,
    /// Block transactions  
    pub transactions: Option<Vec<TxEnvelope>>,
    /// Block info
    pub block_info: BlockInfo,
    /// Whether this block is part of the canonical chain
    pub canonical: bool,
}

impl CachedBlock {
    /// Create a new cached block
    pub fn new(header: Header) -> Self {
        let block_info = BlockInfo {
            hash: header.hash_slow(),
            number: header.number,
            parent_hash: header.parent_hash,
            timestamp: header.timestamp,
        };

        Self { header, receipts: None, transactions: None, block_info, canonical: true }
    }

    /// Set receipts for this block
    pub fn with_receipts(mut self, receipts: Vec<Receipt>) -> Self {
        self.receipts = Some(receipts);
        self
    }

    /// Set transactions for this block
    pub fn with_transactions(mut self, transactions: Vec<TxEnvelope>) -> Self {
        self.transactions = Some(transactions);
        self
    }

    /// Mark this block as non-canonical
    pub fn mark_non_canonical(mut self) -> Self {
        self.canonical = false;
        self
    }
}

/// Buffer for managing chain state with LRU caching and reorg handling
#[derive(Debug)]
pub struct ChainStateBuffer {
    /// LRU cache for blocks by hash
    blocks_by_hash: RwLock<LruCache<B256, CachedBlock>>,
    /// LRU cache for blocks by number
    blocks_by_number: RwLock<LruCache<u64, B256>>,
    /// Current canonical chain head
    canonical_head: RwLock<Option<B256>>,
    /// Maximum reorg depth to support
    max_reorg_depth: u64,
    /// Cache capacity
    capacity: usize,
}

impl ChainStateBuffer {
    /// Create a new chain state buffer
    pub fn new(capacity: usize, max_reorg_depth: u64) -> Self {
        Self {
            blocks_by_hash: RwLock::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
            blocks_by_number: RwLock::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
            canonical_head: RwLock::new(None),
            max_reorg_depth,
            capacity,
        }
    }

    /// Get block by hash from cache
    pub async fn get_block_by_hash(&self, hash: B256) -> Option<CachedBlock> {
        let cache = self.blocks_by_hash.read().await;
        cache.peek(&hash).cloned()
    }

    /// Get block by number from cache
    pub async fn get_block_by_number(&self, number: u64) -> Option<CachedBlock> {
        let blocks_by_number = self.blocks_by_number.read().await;
        if let Some(hash) = blocks_by_number.peek(&number) {
            let hash = *hash;
            drop(blocks_by_number);
            self.get_block_by_hash(hash).await
        } else {
            None
        }
    }

    /// Insert a block into the cache
    pub async fn insert_block(&self, block: CachedBlock) {
        let hash = block.block_info.hash;
        let number = block.block_info.number;

        debug!(
            block_hash = %hash,
            block_number = number,
            canonical = block.canonical,
            "Inserting block into cache"
        );

        let mut blocks_by_hash = self.blocks_by_hash.write().await;
        let mut blocks_by_number = self.blocks_by_number.write().await;

        blocks_by_hash.put(hash, block);
        blocks_by_number.put(number, hash);
    }

    /// Handle a chain state event
    pub async fn handle_event(&self, event: ChainStateEvent) -> Result<(), ChainBufferError> {
        match event {
            ChainStateEvent::ChainCommitted { new_head, committed } => {
                self.handle_chain_committed(new_head, committed).await
            }
            ChainStateEvent::ChainReorged { old_head, new_head, depth } => {
                self.handle_chain_reorged(old_head, new_head, depth).await
            }
            ChainStateEvent::ChainReverted { old_head, new_head, reverted } => {
                self.handle_chain_reverted(old_head, new_head, reverted).await
            }
        }
    }

    /// Handle chain committed event
    async fn handle_chain_committed(
        &self,
        new_head: B256,
        committed: Vec<B256>,
    ) -> Result<(), ChainBufferError> {
        debug!(
            new_head = %new_head,
            committed_count = committed.len(),
            "Handling chain committed event"
        );

        // Update canonical head
        let mut canonical_head = self.canonical_head.write().await;
        *canonical_head = Some(new_head);

        // Mark all committed blocks as canonical
        let mut blocks_by_hash = self.blocks_by_hash.write().await;
        for hash in committed {
            if let Some(block) = blocks_by_hash.get_mut(&hash) {
                block.canonical = true;
            }
        }

        Ok(())
    }

    /// Handle chain reorged event
    async fn handle_chain_reorged(
        &self,
        old_head: B256,
        new_head: B256,
        depth: u64,
    ) -> Result<(), ChainBufferError> {
        warn!(
            old_head = %old_head,
            new_head = %new_head,
            depth = depth,
            max_depth = self.max_reorg_depth,
            "Handling chain reorg event"
        );

        if depth > self.max_reorg_depth {
            return Err(ChainBufferError::ReorgTooDeep { depth, max_depth: self.max_reorg_depth });
        }

        // Update canonical head
        let mut canonical_head = self.canonical_head.write().await;
        *canonical_head = Some(new_head);

        // We need to invalidate cached blocks that are no longer canonical
        // For now, we'll clear the entire cache on deep reorgs
        if depth > 10 {
            warn!(depth = depth, "Deep reorg detected, clearing cache");
            let mut blocks_by_hash = self.blocks_by_hash.write().await;
            let mut blocks_by_number = self.blocks_by_number.write().await;
            blocks_by_hash.clear();
            blocks_by_number.clear();
        }

        Ok(())
    }

    /// Handle chain reverted event
    async fn handle_chain_reverted(
        &self,
        old_head: B256,
        new_head: B256,
        reverted: Vec<B256>,
    ) -> Result<(), ChainBufferError> {
        warn!(
            old_head = %old_head,
            new_head = %new_head,
            reverted_count = reverted.len(),
            "Handling chain reverted event"
        );

        // Update canonical head
        let mut canonical_head = self.canonical_head.write().await;
        *canonical_head = Some(new_head);

        // Mark reverted blocks as non-canonical and remove from cache
        let mut blocks_by_hash = self.blocks_by_hash.write().await;
        for hash in reverted {
            blocks_by_hash.pop(&hash);
        }

        Ok(())
    }

    /// Get the current canonical head
    pub async fn canonical_head(&self) -> Option<B256> {
        let canonical_head = self.canonical_head.read().await;
        *canonical_head
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        let blocks_by_hash = self.blocks_by_hash.read().await;
        let blocks_by_number = self.blocks_by_number.read().await;

        CacheStats {
            blocks_by_hash_len: blocks_by_hash.len(),
            blocks_by_number_len: blocks_by_number.len(),
            capacity: self.capacity,
            max_reorg_depth: self.max_reorg_depth,
        }
    }

    /// Clear the entire cache
    pub async fn clear(&self) {
        let mut blocks_by_hash = self.blocks_by_hash.write().await;
        let mut blocks_by_number = self.blocks_by_number.write().await;
        let mut canonical_head = self.canonical_head.write().await;

        blocks_by_hash.clear();
        blocks_by_number.clear();
        *canonical_head = None;

        debug!("Cleared chain state buffer cache");
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of blocks cached by hash
    pub blocks_by_hash_len: usize,
    /// Number of blocks cached by number
    pub blocks_by_number_len: usize,
    /// Total cache capacity
    pub capacity: usize,
    /// Maximum reorg depth supported
    pub max_reorg_depth: u64,
}

/// Errors that can occur in the chain buffer
#[derive(Debug, thiserror::Error)]
pub enum ChainBufferError {
    /// Reorg is too deep to handle
    #[error("Reorg depth {depth} exceeds maximum supported depth {max_depth}")]
    ReorgTooDeep { depth: u64, max_depth: u64 },
    /// Block not found in cache
    #[error("Block not found in cache: {hash}")]
    BlockNotFound { hash: B256 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{FixedBytes, U256};

    fn create_test_header(number: u64, hash: B256, parent_hash: B256) -> Header {
        Header {
            number,
            parent_hash,
            timestamp: 1234567890,
            gas_limit: 8000000,
            gas_used: 5000000,
            base_fee_per_gas: Some(20_000_000_000u64),
            difficulty: U256::ZERO,
            nonce: FixedBytes::ZERO,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_buffer_basic_operations() {
        let buffer = ChainStateBuffer::new(100, 10);

        let header1 = create_test_header(1, B256::ZERO, B256::ZERO);
        let block1 = CachedBlock::new(header1.clone());
        let computed_hash = block1.block_info.hash;

        // Insert block
        buffer.insert_block(block1.clone()).await;

        // Retrieve by hash
        let retrieved = buffer.get_block_by_hash(computed_hash).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().block_info.hash, computed_hash);

        // Retrieve by number
        let retrieved = buffer.get_block_by_number(1).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().block_info.number, 1);
    }

    #[tokio::test]
    async fn test_chain_committed_event() {
        let buffer = ChainStateBuffer::new(100, 10);

        let hash1 = B256::with_last_byte(1);
        let hash2 = B256::with_last_byte(2);
        let header1 = create_test_header(1, hash1, B256::ZERO);
        let header2 = create_test_header(2, hash2, hash1);

        let block1 = CachedBlock::new(header1);
        let block2 = CachedBlock::new(header2);

        buffer.insert_block(block1).await;
        buffer.insert_block(block2).await;

        // Handle committed event
        let event =
            ChainStateEvent::ChainCommitted { new_head: hash2, committed: vec![hash1, hash2] };

        buffer.handle_event(event).await.unwrap();

        // Check canonical head is updated
        assert_eq!(buffer.canonical_head().await, Some(hash2));
    }

    #[tokio::test]
    async fn test_reorg_too_deep() {
        let buffer = ChainStateBuffer::new(100, 5);

        let hash1 = B256::with_last_byte(1);
        let hash2 = B256::with_last_byte(2);

        let event = ChainStateEvent::ChainReorged {
            old_head: hash1,
            new_head: hash2,
            depth: 10, // Exceeds max depth of 5
        };

        let result = buffer.handle_event(event).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChainBufferError::ReorgTooDeep { .. }));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let buffer = ChainStateBuffer::new(100, 10);

        let hash1 = B256::with_last_byte(1);
        let header1 = create_test_header(1, hash1, B256::ZERO);
        let block1 = CachedBlock::new(header1);

        buffer.insert_block(block1).await;

        let stats = buffer.cache_stats().await;
        assert_eq!(stats.blocks_by_hash_len, 1);
        assert_eq!(stats.blocks_by_number_len, 1);
        assert_eq!(stats.capacity, 100);
        assert_eq!(stats.max_reorg_depth, 10);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let buffer = ChainStateBuffer::new(100, 10);

        let header1 = create_test_header(1, B256::ZERO, B256::ZERO);
        let block1 = CachedBlock::new(header1.clone());
        let computed_hash = block1.block_info.hash;

        buffer.insert_block(block1).await;
        assert!(buffer.get_block_by_hash(computed_hash).await.is_some());

        buffer.clear().await;
        assert!(buffer.get_block_by_hash(computed_hash).await.is_none());
        assert_eq!(buffer.canonical_head().await, None);
    }
}
