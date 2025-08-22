//! Notification handler for processing ExEx chain events.
//!
//! This module handles the processing of chain state events from the ExEx framework,
//! providing a clean interface between the reth ExEx system and the kona-node integration.

use crate::error::{ExExError, RollupResult};
use alloy_primitives::B256;
use std::{collections::HashMap, time::Instant};
use tracing::{debug, error, info, warn};

/// Chain notification types that can be processed.
#[derive(Debug, Clone)]
pub enum ChainNotification {
    /// New blocks have been committed to the canonical chain
    ChainCommitted {
        /// Hash of the new chain head
        new_head: B256,
        /// List of committed block hashes
        committed: Vec<B256>,
    },

    /// Chain reorganization occurred
    ChainReorged {
        /// Hash of the old chain head
        old_head: B256,
        /// Hash of the new chain head
        new_head: B256,
        /// Depth of the reorganization
        depth: u64,
    },

    /// Chain was reverted to a previous state
    ChainReverted {
        /// Hash of the old chain head
        old_head: B256,
        /// Hash of the new chain head after revert
        new_head: B256,
        /// List of reverted block hashes
        reverted: Vec<B256>,
    },
}

/// Handler for processing ExEx chain notifications.
#[derive(Debug, Clone)]
pub struct NotificationHandler {
    /// Processing statistics
    stats: NotificationHandlerStats,

    /// Cache for block heights (hash -> height)
    block_heights: HashMap<B256, u64>,

    /// Last processed height
    last_processed_height: Option<u64>,
}

/// Statistics for the notification handler.
#[derive(Debug, Clone, Default)]
pub struct NotificationHandlerStats {
    /// Number of notifications processed
    pub notifications_processed: u64,
    /// Number of blocks processed
    pub blocks_processed: u64,
    /// Number of reorgs handled
    pub reorgs_handled: u64,
    /// Number of reverts handled
    pub reverts_handled: u64,
    /// Last processing time
    pub last_processed: Option<Instant>,
}

impl NotificationHandler {
    /// Create a new notification handler.
    pub fn new() -> Self {
        Self {
            stats: NotificationHandlerStats::default(),
            block_heights: HashMap::new(),
            last_processed_height: None,
        }
    }

    /// Process a chain notification and return the new height if applicable.
    pub async fn process_notification(
        &self,
        notification: ChainNotification,
    ) -> RollupResult<Option<u64>> {
        debug!("Processing chain notification: {:?}", notification);

        let result = match &notification {
            ChainNotification::ChainCommitted { new_head, committed } => {
                self.handle_chain_committed(*new_head, committed).await
            }
            ChainNotification::ChainReorged { old_head, new_head, depth } => {
                self.handle_chain_reorged(*old_head, *new_head, *depth).await
            }
            ChainNotification::ChainReverted { old_head, new_head, reverted } => {
                self.handle_chain_reverted(*old_head, *new_head, reverted).await
            }
        };

        match result {
            Ok(height) => {
                info!(
                    "Successfully processed {} notification, new height: {:?}",
                    notification.notification_type(),
                    height
                );
                Ok(height)
            }
            Err(e) => {
                error!(
                    "Failed to process {} notification: {}",
                    notification.notification_type(),
                    e
                );
                Err(e)
            }
        }
    }

    /// Handle chain committed notification.
    async fn handle_chain_committed(
        &self,
        new_head: B256,
        committed: &[B256],
    ) -> RollupResult<Option<u64>> {
        info!(
            new_head = %new_head,
            committed_count = committed.len(),
            "Handling chain committed notification"
        );

        // TODO: In a real implementation, we would:
        // 1. Fetch block details from the provider
        // 2. Update our local state/cache
        // 3. Forward relevant information to kona-node
        // 4. Return the actual new height

        // For now, simulate processing
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Simulate determining the new height
        // In practice, this would come from block data
        let new_height = self.estimate_height_from_committed(committed).await?;

        debug!("Chain committed processed, estimated height: {:?}", new_height);
        Ok(new_height)
    }

    /// Handle chain reorg notification.
    async fn handle_chain_reorged(
        &self,
        old_head: B256,
        new_head: B256,
        depth: u64,
    ) -> RollupResult<Option<u64>> {
        warn!(
            old_head = %old_head,
            new_head = %new_head,
            depth = depth,
            "Handling chain reorg notification"
        );

        // Validate reorg depth
        if depth > 1000 {
            return Err(ExExError::reorg_handling(
                depth,
                "Reorg depth too large, manual intervention required",
            )
            .into());
        }

        // TODO: In a real implementation, we would:
        // 1. Revert local state to the common ancestor
        // 2. Apply new chain state
        // 3. Update caches and notify kona-node
        // 4. Return the new chain height

        // For now, simulate reorg processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Simulate determining the new height after reorg
        let new_height = self.estimate_height_from_reorg(depth).await?;

        debug!("Chain reorg processed, estimated height: {:?}", new_height);
        Ok(new_height)
    }

    /// Handle chain reverted notification.
    async fn handle_chain_reverted(
        &self,
        old_head: B256,
        new_head: B256,
        reverted: &[B256],
    ) -> RollupResult<Option<u64>> {
        warn!(
            old_head = %old_head,
            new_head = %new_head,
            reverted_count = reverted.len(),
            "Handling chain reverted notification"
        );

        // TODO: In a real implementation, we would:
        // 1. Remove reverted blocks from our state
        // 2. Update the chain head to new_head
        // 3. Notify kona-node of the revert
        // 4. Return the new chain height

        // For now, simulate revert processing
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

        // Simulate determining the new height after revert
        let new_height = self.estimate_height_from_reverted(reverted).await?;

        debug!("Chain reverted processed, estimated height: {:?}", new_height);
        Ok(new_height)
    }

    /// Estimate height from committed blocks (simulation).
    async fn estimate_height_from_committed(
        &self,
        committed: &[B256],
    ) -> RollupResult<Option<u64>> {
        if committed.is_empty() {
            return Ok(None);
        }

        // In a real implementation, we'd get actual block heights
        // For simulation, assume sequential height increase
        let base_height = self.last_processed_height.unwrap_or(0);
        let new_height = base_height + committed.len() as u64;

        Ok(Some(new_height))
    }

    /// Estimate height from reorg depth (simulation).
    async fn estimate_height_from_reorg(&self, depth: u64) -> RollupResult<Option<u64>> {
        // In a real implementation, we'd calculate from the common ancestor
        let base_height = self.last_processed_height.unwrap_or(0);
        let new_height = base_height.saturating_sub(depth).saturating_add(1);

        Ok(Some(new_height))
    }

    /// Estimate height from reverted blocks (simulation).
    async fn estimate_height_from_reverted(&self, reverted: &[B256]) -> RollupResult<Option<u64>> {
        if reverted.is_empty() {
            return Ok(self.last_processed_height);
        }

        // In a real implementation, we'd calculate from the actual revert
        let base_height = self.last_processed_height.unwrap_or(0);
        let new_height = base_height.saturating_sub(reverted.len() as u64);

        Ok(Some(new_height))
    }

    /// Get processing statistics.
    pub fn get_stats(&self) -> NotificationHandlerStats {
        self.stats.clone()
    }

    /// Check if the handler is healthy.
    pub fn is_healthy(&self) -> bool {
        // In a real implementation, we'd check various health indicators
        // For now, just return true as a placeholder
        true
    }
}

impl Default for NotificationHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ChainNotification {
    /// Get the notification type as a string.
    pub fn notification_type(&self) -> &'static str {
        match self {
            ChainNotification::ChainCommitted { .. } => "committed",
            ChainNotification::ChainReorged { .. } => "reorged",
            ChainNotification::ChainReverted { .. } => "reverted",
        }
    }

    /// Get the new head hash from the notification.
    pub fn new_head(&self) -> B256 {
        match self {
            ChainNotification::ChainCommitted { new_head, .. } => *new_head,
            ChainNotification::ChainReorged { new_head, .. } => *new_head,
            ChainNotification::ChainReverted { new_head, .. } => *new_head,
        }
    }

    /// Get the number of blocks affected by this notification.
    pub fn affected_blocks_count(&self) -> usize {
        match self {
            ChainNotification::ChainCommitted { committed, .. } => committed.len(),
            ChainNotification::ChainReorged { depth, .. } => *depth as usize,
            ChainNotification::ChainReverted { reverted, .. } => reverted.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;

    #[tokio::test]
    async fn test_notification_handler_creation() {
        let handler = NotificationHandler::new();
        assert!(handler.is_healthy());
        assert_eq!(handler.get_stats().notifications_processed, 0);
    }

    #[tokio::test]
    async fn test_chain_committed_processing() {
        let handler = NotificationHandler::new();

        let notification = ChainNotification::ChainCommitted {
            new_head: B256::random(),
            committed: vec![B256::random(), B256::random()],
        };

        let result = handler.process_notification(notification).await;
        assert!(result.is_ok());

        let height = result.unwrap();
        assert!(height.is_some());
        assert_eq!(height.unwrap(), 2); // Simulated: 0 + 2 committed blocks
    }

    #[tokio::test]
    async fn test_chain_reorg_processing() {
        let handler = NotificationHandler::new();

        let notification = ChainNotification::ChainReorged {
            old_head: B256::random(),
            new_head: B256::random(),
            depth: 3,
        };

        let result = handler.process_notification(notification).await;
        assert!(result.is_ok());

        let height = result.unwrap();
        assert!(height.is_some());
    }

    #[tokio::test]
    async fn test_chain_reverted_processing() {
        let handler = NotificationHandler::new();

        let notification = ChainNotification::ChainReverted {
            old_head: B256::random(),
            new_head: B256::random(),
            reverted: vec![B256::random()],
        };

        let result = handler.process_notification(notification).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_deep_reorg_error() {
        let handler = NotificationHandler::new();

        let notification = ChainNotification::ChainReorged {
            old_head: B256::random(),
            new_head: B256::random(),
            depth: 2000, // Too deep
        };

        let result = handler.process_notification(notification).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_chain_notification_methods() {
        let new_head = B256::random();

        let committed_notification = ChainNotification::ChainCommitted {
            new_head,
            committed: vec![B256::random(), B256::random()],
        };

        assert_eq!(committed_notification.notification_type(), "committed");
        assert_eq!(committed_notification.new_head(), new_head);
        assert_eq!(committed_notification.affected_blocks_count(), 2);

        let reorg_notification =
            ChainNotification::ChainReorged { old_head: B256::random(), new_head, depth: 5 };

        assert_eq!(reorg_notification.notification_type(), "reorged");
        assert_eq!(reorg_notification.new_head(), new_head);
        assert_eq!(reorg_notification.affected_blocks_count(), 5);

        let reverted_notification = ChainNotification::ChainReverted {
            old_head: B256::random(),
            new_head,
            reverted: vec![B256::random(), B256::random(), B256::random()],
        };

        assert_eq!(reverted_notification.notification_type(), "reverted");
        assert_eq!(reverted_notification.new_head(), new_head);
        assert_eq!(reverted_notification.affected_blocks_count(), 3);
    }

    #[test]
    fn test_notification_handler_stats() {
        let handler = NotificationHandler::new();
        let stats = handler.get_stats();

        assert_eq!(stats.notifications_processed, 0);
        assert_eq!(stats.blocks_processed, 0);
        assert_eq!(stats.reorgs_handled, 0);
        assert_eq!(stats.reverts_handled, 0);
        assert!(stats.last_processed.is_none());
    }
}
