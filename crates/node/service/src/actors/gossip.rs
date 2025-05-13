//! Gossip Actor

use crate::NodeActor;
use async_trait::async_trait;
use discv5::Enr;
use kona_gossip::Driver;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use thiserror::Error;
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;

/// The gossip actor manages a libp2p gossipsub swarm that shares unsafe L2 blocks
/// over the TCP network.
///
/// The gossip actor itself is a light wrapper around the [`Driver`].
#[derive(Debug)]
pub struct GossipActor {
    /// The gossip driver.
    driver: Driver,
    /// A channel to receive [`Enr`]s from the discovery actor.
    enr_receiver: Receiver<Enr>,
    /// A channel to receive [`OpNetworkPayloadEnvelope`]s.
    payload_rx: Receiver<OpNetworkPayloadEnvelope>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl GossipActor {
    /// Constructs a new [`GossipActor`] given the [`Driver`]
    pub const fn new(
        driver: Driver,
        enr_receiver: Receiver<Enr>,
        payload_rx: Receiver<OpNetworkPayloadEnvelope>,
        cancellation: CancellationToken,
    ) -> Self {
        Self { driver, enr_receiver, payload_rx, cancellation }
    }
}

#[async_trait]
impl NodeActor for GossipActor {
    type InboundEvent = ();
    type Error = GossipActorError;

    async fn start(mut self) -> Result<(), Self::Error> {
        let handle = self.driver.start();
        loop {
            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    info!(
                        target: "discovery",
                        "Received shutdown signal. Exiting discovery task."
                    );
                    return Ok(());
                }
                Some(enr) = self.enr_receiver.recv() => {
                    handle.send_enr(enr);
                }
                Some(payload) = self.payload_rx.recv() => {
                    handle.send_payload(payload);
                }
            }
        }
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// An error from the gossip actor.
#[derive(Debug, Error)]
pub enum GossipActorError {}
