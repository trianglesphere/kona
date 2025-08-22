//! # ExEx Integration Module
//!
//! This module implements the integration of kona-node as an execution extension (ExEx)
//! within the op-reth framework, enabling seamless operation of the rollup logic as part
//! of the unified rollup binary.

mod notification_handler;
mod types;

pub use notification_handler::{ChainNotification, NotificationHandler};
pub use types::{ExExMetrics, ExExState};

use crate::{
    config::ExExConfig,
    error::{ExExError, RollupResult},
};
use alloy_consensus::Header;
use alloy_primitives::B256;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// The main Kona ExEx implementation.
///
/// This struct implements the reth ExEx trait and serves as the integration point
/// between the op-reth node and the kona-node service.
pub struct KonaNodeExEx {
    /// ExEx configuration
    config: ExExConfig,

    /// Notification handler
    notification_handler: NotificationHandler,

    /// Metrics collector
    metrics: Arc<Mutex<ExExMetrics>>,

    /// Current processing state
    state: Arc<Mutex<ExExState>>,

    /// Current processing height
    current_height: Arc<Mutex<Option<u64>>>,

    /// Notification channel for processing
    notification_tx: mpsc::UnboundedSender<ChainNotification>,
    notification_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<ChainNotification>>>>,
}

impl KonaNodeExEx {
    /// Create a new Kona ExEx instance.
    ///
    /// # Arguments
    /// * `config` - ExEx configuration
    ///
    /// # Returns
    /// * `Result<KonaNodeExEx>` - New ExEx instance or error
    ///
    /// # Example
    /// ```rust,no_run
    /// use rollup_service::{ExExConfig, KonaNodeExEx};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = ExExConfig::default();
    ///     let exex = KonaNodeExEx::new(config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(config: ExExConfig) -> RollupResult<Self> {
        info!("Creating new Kona ExEx instance");

        // Create notification handler
        let notification_handler = NotificationHandler::new();

        // Initialize metrics and state
        let metrics = Arc::new(Mutex::new(ExExMetrics::default()));
        let state = Arc::new(Mutex::new(ExExState::new()));
        let current_height = Arc::new(Mutex::new(None));

        // Create notification channel
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();
        let notification_rx = Arc::new(Mutex::new(Some(notification_rx)));

        info!("Kona ExEx instance created with config: {:?}", config);

        Ok(Self {
            config,
            notification_handler,
            metrics,
            state,
            current_height,
            notification_tx,
            notification_rx,
        })
    }

    /// Start the ExEx processing loop.
    ///
    /// This method should be called to begin processing chain notifications.
    pub async fn start(&mut self) -> RollupResult<()> {
        info!("Starting Kona ExEx processing loop");

        // Mark as running
        {
            let mut state = self.state.lock().unwrap();
            state.start();
        }

        // Take the receiver out to avoid borrowing issues
        let mut notification_rx = {
            let mut rx_guard = self.notification_rx.lock().unwrap();
            rx_guard.take().ok_or_else(|| ExExError::lifecycle("start", "Already started"))?
        };

        // Start processing notifications
        let notification_handler = self.notification_handler.clone();
        let metrics = Arc::clone(&self.metrics);
        let state = Arc::clone(&self.state);
        let current_height = Arc::clone(&self.current_height);

        tokio::spawn(async move {
            while let Some(notification) = notification_rx.recv().await {
                debug!("Processing ExEx notification");

                // Update metrics
                {
                    let mut metrics = metrics.lock().unwrap();
                    metrics.increment_notifications();
                }

                // Process notification
                match notification_handler.process_notification(notification).await {
                    Ok(height) => {
                        // Update current height
                        if let Some(h) = height {
                            let mut height_guard = current_height.lock().unwrap();
                            *height_guard = Some(h);

                            let mut state = state.lock().unwrap();
                            state.update_height(h);
                            state.clear_error();
                        }

                        debug!("Successfully processed notification");
                    }
                    Err(e) => {
                        error!("Failed to process notification: {}", e);

                        let mut state = state.lock().unwrap();
                        state.record_error(e.to_string());
                    }
                }
            }

            info!("ExEx notification processing loop ended");
        });

        info!("Kona ExEx started successfully");
        Ok(())
    }

    /// Process a chain notification.
    ///
    /// This method is called by the reth framework to process chain events.
    pub async fn process_notification(&self, notification: ChainNotification) -> RollupResult<()> {
        debug!("Queueing chain notification for processing: {:?}", notification);

        self.notification_tx.send(notification).map_err(|e| {
            ExExError::communication(format!("Failed to queue notification: {}", e))
        })?;

        Ok(())
    }

    /// Get current processing height.
    ///
    /// # Returns
    /// * `Option<u64>` - Current processing height or None if not yet initialized
    pub fn current_height(&self) -> Option<u64> {
        let height = self.current_height.lock().unwrap();
        *height
    }

    /// Get current ExEx metrics.
    ///
    /// # Returns
    /// * `ExExMetrics` - Current metrics snapshot
    pub fn get_metrics(&self) -> ExExMetrics {
        let metrics = self.metrics.lock().unwrap();
        metrics.clone()
    }

    /// Get current ExEx state.
    ///
    /// # Returns
    /// * `ExExState` - Current state snapshot
    pub fn get_state(&self) -> ExExState {
        let state = self.state.lock().unwrap();
        state.clone()
    }

    /// Check if the ExEx is healthy.
    ///
    /// # Returns
    /// * `bool` - True if ExEx is operating normally
    pub fn is_healthy(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.running && state.last_error.is_none()
    }

    /// Shutdown the ExEx gracefully.
    pub async fn shutdown(&mut self) -> RollupResult<()> {
        info!("Shutting down Kona ExEx");

        {
            let mut state = self.state.lock().unwrap();
            state.stop();
        }

        info!("Kona ExEx shutdown completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;

    #[tokio::test]
    async fn test_exex_creation() {
        let config = ExExConfig::default();
        let exex = KonaNodeExEx::new(config).await.unwrap();

        assert!(!exex.is_healthy()); // Should not be healthy until started
        assert_eq!(exex.current_height(), None);
    }

    #[tokio::test]
    async fn test_exex_lifecycle() {
        let config = ExExConfig::default();
        let mut exex = KonaNodeExEx::new(config).await.unwrap();

        assert!(!exex.get_state().running);

        // Start the ExEx
        exex.start().await.unwrap();
        assert!(exex.get_state().running);
        assert!(exex.is_healthy());

        // Shutdown the ExEx
        exex.shutdown().await.unwrap();
        assert!(!exex.get_state().running);
    }

    #[tokio::test]
    async fn test_notification_processing() {
        let config = ExExConfig::default();
        let mut exex = KonaNodeExEx::new(config).await.unwrap();

        exex.start().await.unwrap();

        let notification = ChainNotification::ChainCommitted {
            new_head: B256::random(),
            committed: vec![B256::random(), B256::random()],
        };

        let result = exex.process_notification(notification).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_chain_notification_methods() {
        let new_head = B256::random();
        let notification =
            ChainNotification::ChainCommitted { new_head, committed: vec![B256::random()] };

        assert_eq!(notification.new_head(), new_head);
        assert_eq!(notification.notification_type(), "committed");

        let reorg_notification =
            ChainNotification::ChainReorged { old_head: B256::random(), new_head, depth: 5 };

        assert_eq!(reorg_notification.new_head(), new_head);
        assert_eq!(reorg_notification.notification_type(), "reorged");

        let revert_notification = ChainNotification::ChainReverted {
            old_head: B256::random(),
            new_head,
            reverted: vec![B256::random()],
        };

        assert_eq!(revert_notification.new_head(), new_head);
        assert_eq!(revert_notification.notification_type(), "reverted");
    }

    #[tokio::test]
    async fn test_exex_metrics() {
        let config = ExExConfig::default();
        let exex = KonaNodeExEx::new(config).await.unwrap();

        let metrics = exex.get_metrics();
        assert_eq!(metrics.notifications_processed, 0);
        assert_eq!(metrics.blocks_processed, 0);
        assert_eq!(metrics.errors, 0);
    }
}
