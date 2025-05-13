//! Discovery Actor

use crate::NodeActor;
use async_trait::async_trait;
use discv5::Enr;
use kona_disc::Discv5Driver;
use thiserror::Error;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

/// The discovery actor handles peer discovery over UDP using discv5.
///
/// The discovery actor itself is a light wrapper around the [`Discv5Driver`].
#[derive(Debug)]
pub struct DiscoveryActor {
    /// The discovery driver.
    driver: Discv5Driver,
    /// A channel to forward discovered [`Enr`]s to the gossip actor.
    enr_sender: Sender<Enr>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl DiscoveryActor {
    /// Constructs a new [`DiscoveryActor`] given the [`Discv5Driver`]
    pub const fn new(
        driver: Discv5Driver,
        enr_sender: Sender<Enr>,
        cancellation: CancellationToken,
    ) -> Self {
        Self { driver, enr_sender, cancellation }
    }
}

#[async_trait]
impl NodeActor for DiscoveryActor {
    type InboundEvent = ();
    type Error = DiscoveryActorError;

    async fn start(mut self) -> Result<(), Self::Error> {
        let (handler, mut enr_receiver) = self.driver.start();
        loop {
            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    info!(
                        target: "discovery",
                        "Received shutdown signal. Exiting discovery task."
                    );
                    return Ok(());
                }
                Some(enr) = enr_receiver.recv() => {
                    self.enr_sender.send(enr).await.unwrap_or_else(|e| {
                        tracing::warn!("Failed to send ENR to gossip actor: {:?}", e);
                    });
                }
            }
        }
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// An error from the discovery actor.
#[derive(Debug, Error)]
pub enum DiscoveryActorError {}
