//! Contains a handle to the spawned gossip driver.

use discv5::Enr;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use tokio::sync::mpsc::Sender;

/// Handler to the spawned [`libp2p::Swarm`] service.
///
/// Provides a lock-free way to access the spawned swarm
/// by using message-passing to relay requests and responses
/// through channels.
#[derive(Debug, Clone)]
pub struct GossipHandle {
    /// Used to send an [`Enr`] to the spawned [`libp2p::Swarm`] service.
    enr_sender: Sender<Enr>,
    /// Used to send an [`OpNetworkPayloadEnvelope`] to the spawned [`libp2p::Swarm`] service.
    payload_sender: Sender<OpNetworkPayloadEnvelope>,
}

impl GossipHandle {
    /// Creates a new [`GossipHandle`] service.
    pub const fn new(
        enr_sender: Sender<Enr>,
        payload_sender: Sender<OpNetworkPayloadEnvelope>,
    ) -> Self {
        Self { enr_sender, payload_sender }
    }

    /// Sends an [`Enr`] to the spawned [`libp2p::Swarm`] service.
    pub fn send_enr(&self, enr: Enr) {
        let sender = self.enr_sender.clone();
        tokio::spawn(async move {
            if let Err(e) = sender.send(enr).await {
                warn!("Failed to send ENR: {:?}", e);
            }
        });
    }

    /// Sends an [`OpNetworkPayloadEnvelope`] to the spawned [`libp2p::Swarm`] service.
    pub fn send_payload(&self, payload: OpNetworkPayloadEnvelope) {
        let sender = self.payload_sender.clone();
        tokio::spawn(async move {
            if let Err(e) = sender.send(payload).await {
                warn!("Failed to send payload: {:?}", e);
            }
        });
    }
}
