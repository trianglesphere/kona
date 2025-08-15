//! Test helpers for the derivation actor.

use kona_derive::Signal;
use kona_node_service::{DerivationError, DerivationInboundChannels};
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

pub(crate) mod builder;
pub(crate) mod pipeline;

pub(crate) use builder::TestDerivationBuilder;
use tokio_util::sync::CancellationToken;

pub struct TestDerivationChannels {
    pub l1_head_updates_tx: watch::Sender<Option<BlockInfo>>,
    pub engine_l2_safe_head_tx: watch::Sender<L2BlockInfo>,
    /// We are going to put a dummy value here once the EL sync is complete.
    pub el_sync_complete_tx: Option<oneshot::Sender<()>>,
    pub derivation_signal_tx: mpsc::Sender<Signal>,
}

impl From<DerivationInboundChannels> for TestDerivationChannels {
    fn from(inbound: DerivationInboundChannels) -> Self {
        Self {
            l1_head_updates_tx: inbound.l1_head_updates_tx,
            engine_l2_safe_head_tx: inbound.engine_l2_safe_head_tx,
            el_sync_complete_tx: Some(inbound.el_sync_complete_tx),
            derivation_signal_tx: inbound.derivation_signal_tx,
        }
    }
}

/// Test harness for the derivation actor.
pub(crate) struct TestDerivation {
    /// The inbound channels for sending messages to the derivation actor.
    pub(super) inbound: TestDerivationChannels,
    /// Receiver for derived attributes produced by the derivation actor.
    pub(super) attributes_rx: mpsc::Receiver<OpAttributesWithParent>,
    /// Receiver for reset requests from the derivation actor.
    pub(super) reset_rx: mpsc::Receiver<()>,
    /// The task handle for the derivation actor.
    #[allow(dead_code)]
    pub(super) handle: JoinHandle<Result<(), DerivationError>>,
    /// The cancellation token for the derivation actor.
    pub(super) cancellation: CancellationToken,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum TestDerivationError {
    #[error("Derivation actor channel closed")]
    ChannelClosed,
    #[error("Failed to receive response: {0}")]
    OneshotError(#[from] oneshot::error::RecvError),
    #[error("Failed to send signal: {0}")]
    SignalSendError(String),
    #[error("No attributes available")]
    NoAttributesAvailable,
}

impl TestDerivation {
    /// Note: This function can only be called once as it consumes the oneshot sender.
    /// The el_sync_complete channel is consumed when calling this.
    pub(super) fn signal_el_sync_complete(&mut self) -> anyhow::Result<()> {
        self.inbound
            .el_sync_complete_tx
            .take()
            .ok_or(anyhow::anyhow!("El sync complete tx not set"))?
            .send(())
            .map_err(|_| anyhow::anyhow!("Failed to send el sync complete signal"))?;

        Ok(())
    }

    /// Updates the L1 head for the derivation actor.
    pub(super) fn update_l1_head(
        &self,
        block: Option<BlockInfo>,
    ) -> Result<(), TestDerivationError> {
        self.inbound.l1_head_updates_tx.send(block).map_err(|_| TestDerivationError::ChannelClosed)
    }

    /// Spin off the derivation actor and return the handle's result.
    pub(super) async fn wait(self) -> Result<(), DerivationError> {
        self.cancellation.cancel();

        self.handle.await.expect("Join handle panicked")
    }

    /// Updates the L2 safe head for the derivation actor.
    pub(super) fn update_l2_safe_head(
        &self,
        block: L2BlockInfo,
    ) -> Result<(), TestDerivationError> {
        self.inbound
            .engine_l2_safe_head_tx
            .send(block)
            .map_err(|_| TestDerivationError::ChannelClosed)
    }

    /// Sends a signal to the derivation pipeline.
    pub(super) async fn send_signal(&self, signal: Signal) -> Result<(), TestDerivationError> {
        self.inbound
            .derivation_signal_tx
            .send(signal)
            .await
            .map_err(|e| TestDerivationError::SignalSendError(e.to_string()))
    }

    /// Attempts to receive the next derived attributes.
    pub(super) async fn recv_attributes(
        &mut self,
    ) -> Result<OpAttributesWithParent, TestDerivationError> {
        self.attributes_rx.recv().await.ok_or(TestDerivationError::NoAttributesAvailable)
    }

    /// Attempts to receive the next derived attributes with a timeout.
    pub(super) async fn recv_attributes_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<OpAttributesWithParent, TestDerivationError> {
        tokio::time::timeout(timeout, self.recv_attributes())
            .await
            .map_err(|_| TestDerivationError::NoAttributesAvailable)?
    }

    /// Checks if there's a reset request pending.
    pub(super) async fn has_reset_request(&mut self) -> bool {
        self.reset_rx.try_recv().is_ok()
    }

    /// Gets the current L1 head from the derivation actor.
    pub(super) fn current_l1_head(&self) -> Option<BlockInfo> {
        self.inbound.l1_head_updates_tx.borrow().clone()
    }

    /// Gets the current L2 safe head from the derivation actor.
    pub(super) fn current_l2_safe_head(&self) -> L2BlockInfo {
        self.inbound.engine_l2_safe_head_tx.borrow().clone()
    }
}
