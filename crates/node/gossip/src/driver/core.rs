//! Consensus-layer gossip driver for the OP Stack.

use derive_more::Debug;
use discv5::Enr;
use futures::stream::StreamExt;
use libp2p::{
    Multiaddr, PeerId, Swarm, TransportError,
    gossipsub::{IdentTopic, MessageId},
    swarm::SwarmEvent,
};
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::collections::{HashMap, HashSet};
use tokio::time::Duration;

use crate::{
    Behaviour, BlockHandler, BlockValidator, Broadcast, Builder, Event, GossipHandle, Handler,
    PublishError,
};
use kona_peers::{EnrValidation, PeerMonitoring, enr_to_multiaddr};

/// A driver for a [`Swarm`] instance.
///
/// Connects the swarm to the given [`Multiaddr`]
/// and handles events using the [`BlockHandler`].
#[derive(Debug)]
pub struct Driver {
    /// The [`Swarm`] instance.
    #[debug(skip)]
    pub swarm: Swarm<Behaviour>,
    /// A [`Multiaddr`] to listen on.
    pub addr: Multiaddr,
    /// The [`BlockHandler`].
    pub handler: BlockHandler<BlockValidator>,
    /// The broadcast handler to broadcast unsafe payloads.
    pub(crate) broadcast: Broadcast,
    /// A mapping from [`Multiaddr`] to the number of times it has been dialed.
    ///
    /// A peer cannot be redialed more than [`GossipDriverBuilder.peer_redialing`] times.
    pub dialed_peers: HashMap<Multiaddr, u64>,
    /// A mapping from [`PeerId`] to [`Multiaddr`].
    pub peerstore: HashMap<PeerId, Multiaddr>,
    /// If set, the gossip layer will monitor peer scores and ban peers that are below a given
    /// threshold.
    pub peer_monitoring: Option<PeerMonitoring>,
    /// The number of times to redial a peer.
    pub peer_redialing: Option<u64>,
}

impl Driver {
    /// The frequency at which to inspect peer scores to ban poorly performing peers.
    const PEER_SCORE_INSPECT_FREQUENCY: Duration = Duration::from_secs(1);

    /// Returns the [`Builder`] that can be used to construct the [`Driver`].
    pub const fn builder() -> Builder {
        Builder::new()
    }

    /// Creates a new [`Driver`] instance.
    pub fn new(
        swarm: Swarm<Behaviour>,
        addr: Multiaddr,
        redialing: Option<u64>,
        handler: BlockHandler<BlockValidator>,
        broadcast: Broadcast,
    ) -> Self {
        Self {
            swarm,
            addr,
            handler,
            broadcast,
            dialed_peers: Default::default(),
            peerstore: Default::default(),
            peer_monitoring: None,
            peer_redialing: redialing,
        }
    }

    /// Publishes an unsafe block to gossip.
    ///
    /// ## Arguments
    ///
    /// * `topic_selector` - A function that selects the topic for the block. This is expected to be
    ///   a closure that takes the [`BlockHandler`] and returns the [`IdentTopic`] for the block.
    /// * `payload` - The payload to be published.
    ///
    /// ## Returns
    ///
    /// Returns the [`MessageId`] of the published message or a [`PublishError`]
    /// if the message could not be published.
    pub fn publish(
        &mut self,
        selector: impl FnOnce(&BlockHandler<BlockValidator>) -> IdentTopic,
        payload: Option<OpNetworkPayloadEnvelope>,
    ) -> Result<Option<MessageId>, PublishError> {
        let Some(payload) = payload else {
            return Ok(None);
        };
        let topic = selector(&self.handler);
        let topic_hash = topic.hash();
        let data = self.handler.encode(topic, payload)?;
        let id = self.swarm.behaviour_mut().gossipsub.publish(topic_hash, data)?;
        kona_macros::inc!(gauge, crate::Metrics::UNSAFE_BLOCK_PUBLISHED);
        Ok(Some(id))
    }

    /// Tells the swarm to listen on the given [`Multiaddr`].
    /// Waits for the swarm to start listen before returning and connecting to peers.
    pub async fn listen(&mut self) -> Result<(), TransportError<std::io::Error>> {
        match self.swarm.listen_on(self.addr.clone()) {
            Ok(id) => loop {
                if let SwarmEvent::NewListenAddr { address, listener_id } =
                    self.swarm.select_next_some().await
                {
                    if id == listener_id {
                        info!(target: "gossip", "Swarm now listening on: {address}");
                        break;
                    }
                }
            },
            Err(err) => {
                error!(target: "gossip", "Fail to listen on {}: {err}", self.addr);
                return Err(err);
            }
        }
        Ok(())
    }

    /// Returns the local peer id.
    pub fn local_peer_id(&self) -> &libp2p::PeerId {
        self.swarm.local_peer_id()
    }

    /// Returns a mutable reference to the Swarm's behaviour.
    pub fn behaviour_mut(&mut self) -> &mut Behaviour {
        self.swarm.behaviour_mut()
    }

    /// Attempts to select the next event from the Swarm.
    pub async fn select_next_some(&mut self) -> SwarmEvent<Event> {
        self.swarm.select_next_some().await
    }

    /// Returns if the given [`Multiaddr`] has been dialed the maximum number of times.
    pub fn dial_threshold_reached(&mut self, addr: &Multiaddr) -> bool {
        // If the peer has not been dialed yet, the threshold is not reached.
        let Some(dialed) = self.dialed_peers.get(addr) else {
            return false;
        };
        // If the peer has been dialed and the threshold is not set, the threshold is reached.
        let Some(redialing) = self.peer_redialing else {
            return true;
        };
        // If the threshold is set to `0`, redial indefinitely.
        if redialing == 0 {
            return false;
        }
        if *dialed >= redialing {
            return true;
        }
        false
    }

    /// Returns the number of connected peers.
    pub fn connected_peers(&self) -> usize {
        self.swarm.connected_peers().count()
    }

    /// Dials the given [`Enr`].
    pub fn dial(&mut self, enr: Enr) {
        kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER);
        let validation = EnrValidation::validate(&enr, self.handler.rollup_config.l2_chain_id);
        if validation.is_invalid() {
            trace!(target: "gossip", "Invalid OP Stack ENR for chain id {}: {}", self.handler.rollup_config.l2_chain_id, validation);
            return;
        }
        let Some(multiaddr) = enr_to_multiaddr(&enr) else {
            debug!(target: "gossip", "Failed to extract tcp socket from enr: {:?}", enr);
            return;
        };
        self.dial_multiaddr(multiaddr);
    }

    /// Dials the given [`Multiaddr`].
    pub fn dial_multiaddr(&mut self, addr: Multiaddr) {
        if self.dial_threshold_reached(&addr) {
            event!(tracing::Level::TRACE, peer=%addr, "Dial threshold reached, not dialing");
            return;
        }

        match self.swarm.dial(addr.clone()) {
            Ok(_) => {
                trace!(target: "gossip", peer=?addr, "Dialed peer");
                let count = self.dialed_peers.entry(addr.clone()).or_insert(0);
                *count += 1;
            }
            Err(e) => {
                error!(target: "gossip", "Failed to connect to peer: {:?}", e);
            }
        }
    }

    /// Redials the given [`PeerId`] using the peerstore.
    pub fn redial(&mut self, peer_id: PeerId) {
        if let Some(addr) = self.peerstore.get(&peer_id) {
            trace!(target: "gossip", "Redialing peer with id: {:?}", peer_id);
            self.dial_multiaddr(addr.clone());
        }
    }

    /// Handles a [`libp2p::gossipsub::Event`].
    fn handle_gossipsub_event(
        &mut self,
        event: libp2p::gossipsub::Event,
    ) -> Option<OpNetworkPayloadEnvelope> {
        match event {
            libp2p::gossipsub::Event::Message {
                propagation_source: src,
                message_id: id,
                message,
            } => {
                trace!(target: "gossip", "Received message with topic: {}", message.topic);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIP_EVENT,
                    "message",
                    message.topic.to_string()
                );
                if self.handler.topics().contains(&message.topic) {
                    let (status, payload) = self.handler.handle(message);
                    _ = self
                        .swarm
                        .behaviour_mut()
                        .gossipsub
                        .report_message_validation_result(&id, &src, status);
                    return payload;
                }
            }
            libp2p::gossipsub::Event::Subscribed { peer_id, topic } => {
                trace!(target: "gossip", "Peer: {:?} subscribed to topic: {:?}", peer_id, topic);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIP_EVENT,
                    "subscribed",
                    topic.to_string()
                );
            }
            libp2p::gossipsub::Event::Unsubscribed { peer_id, topic } => {
                trace!(target: "gossip", "Peer: {:?} unsubscribed from topic: {:?}", peer_id, topic);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIP_EVENT,
                    "unsubscribed",
                    topic.to_string()
                );
            }
            libp2p::gossipsub::Event::SlowPeer { peer_id, .. } => {
                trace!(target: "gossip", "Slow peer: {:?}", peer_id);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIP_EVENT,
                    "slow_peer",
                    peer_id.to_string()
                );
            }
            libp2p::gossipsub::Event::GossipsubNotSupported { peer_id } => {
                trace!(target: "gossip", "Peer: {:?} does not support gossipsub", peer_id);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIP_EVENT,
                    "not_supported",
                    peer_id.to_string()
                );
            }
        }
        None
    }

    /// Handles the [`SwarmEvent<Event>`].
    pub fn handle_event(&mut self, event: SwarmEvent<Event>) -> Option<OpNetworkPayloadEnvelope> {
        kona_macros::inc!(gauge, crate::Metrics::GOSSIP_EVENT, "total", "total");
        let event = match event {
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                let peer_count = self.swarm.connected_peers().count();
                debug!(target: "gossip", "Connection established: {:?} | Peer Count: {}", peer_id, peer_count);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIPSUB_CONNECTION,
                    "connected",
                    peer_id.to_string()
                );
                kona_macros::set!(gauge, crate::Metrics::GOSSIP_PEER_COUNT, peer_count as f64);
                self.peerstore.insert(peer_id, endpoint.get_remote_address().clone());
                return None;
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                debug!(target: "gossip", "Outgoing connection error: {:?}", error);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIPSUB_CONNECTION,
                    "outgoing_error",
                    peer_id.map(|p| p.to_string()).unwrap_or_default()
                );
                if let Some(id) = peer_id {
                    self.redial(id);
                }
                return None;
            }
            SwarmEvent::IncomingConnectionError { error, connection_id, .. } => {
                trace!(target: "gossip", ?error, ?connection_id, "Incoming connection error");
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIPSUB_CONNECTION,
                    "incoming_error",
                    connection_id.to_string()
                );
                return None;
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                let peer_count = self.swarm.connected_peers().count();
                debug!(target: "gossip", "Connection closed, redialing peer: {:?} | {:?} | Peer Count: {}", peer_id, cause, peer_count);
                kona_macros::inc!(gauge, crate::Metrics::GOSSIPSUB_CONNECTION, "type", "closed");
                kona_macros::set!(gauge, crate::Metrics::GOSSIP_PEER_COUNT, peer_count as f64);
                self.redial(peer_id);
                return None;
            }
            SwarmEvent::NewListenAddr { listener_id, address } => {
                debug!(target: "gossip", reporter_id = ?listener_id, new_address = ?address, "New listen address");
                return None;
            }
            SwarmEvent::Behaviour(event) => event,
            _ => {
                debug!(target: "gossip", ?event, "Ignoring non-behaviour in event handler");
                return None;
            }
        };

        match event {
            Event::Ping(libp2p::ping::Event { peer, result, .. }) => {
                trace!(target: "gossip", ?peer, ?result, "Ping received");
                None
            }
            Event::Gossipsub(e) => self.handle_gossipsub_event(e),
            Event::Identify(e) => {
                trace!(target: "gossip", event = ?e, "Identify event received");
                None
            }
        }
    }

    /// Spawns a new task for the gossip driver.
    ///
    /// Returns a [`GossipHandle`] that can be used to
    /// interact with the driver through channels.
    pub fn start(mut self) -> GossipHandle {
        let (block_tx, mut block_rx) = tokio::sync::mpsc::channel::<OpNetworkPayloadEnvelope>(1024);
        let (enr_sender, mut enr_receiver) = tokio::sync::mpsc::channel::<Enr>(1024);

        // We are checking the peer scores every [`Self::PEER_SCORE_INSPECT_FREQUENCY`] seconds.
        let mut peer_score_inspector = tokio::time::interval(Self::PEER_SCORE_INSPECT_FREQUENCY);

        tokio::spawn(async move {
            // TODO: backon with exponential backoff
            if let Err(e) = self.listen().await {
                error!(target: "gossip", ?e, "Failed to listen on {}", self.addr);
                return;
            }
            loop {
                self.broadcast.broadcast();
                tokio::select! {
                    Some(block) = block_rx.recv() => {
                        let timestamp = block.payload.timestamp();
                        let selector = |handler: &crate::BlockHandler<crate::BlockValidator>| {
                            handler.topic(timestamp)
                        };
                        match self.publish(selector, Some(block)) {
                            Ok(id) => info!(target: "gossip", ?id, "Published unsafe payload"),
                            Err(e) => warn!(target: "gossip", ?e, "Failed to publish unsafe payload"),
                        }
                    }
                    event = self.select_next_some() => {
                        if let Some(payload) = self.handle_event(event) {
                            self.broadcast.push(payload);
                        }
                    }
                    Some(enr) = enr_receiver.recv() => {
                        self.dial(enr);
                    },
                    _ = peer_score_inspector.tick(), if self.peer_monitoring.as_ref().is_some() => {
                        // Inspect peer scores and ban peers that are below the threshold.
                        let Some(ban_peers) = self.peer_monitoring.as_ref() else {
                            continue;
                        };

                        // We iterate over all connected peers and check their scores.
                        // We collect a list of peers to remove
                        let peers_to_remove = self.swarm.connected_peers().filter_map(
                            |peer_id| {
                                 // If the score is not available, we use a default value of 0.
                                 let score = self.swarm.behaviour().gossipsub.peer_score(peer_id).unwrap_or_default();

                                 if score < ban_peers.ban_threshold {
                                    return Some(*peer_id);
                                 }

                                 None
                            }
                        ).collect::<Vec<_>>();

                        // We remove the addresses from the gossip layer.
                        let _addrs_to_ban = peers_to_remove.into_iter().filter_map(|peer_to_remove| {
                            // In that case, we ban the peer. This means...
                            // 1. We remove the peer from the network gossip.
                            // 2. We ban the peer from the discv5 service.
                            if self.swarm.disconnect_peer_id(peer_to_remove).is_err() {
                                warn!(peer = ?peer_to_remove, "Trying to disconnect a non-existing peer from the gossip driver.");
                            }

                            if let Some(addr) = self.peerstore.remove(&peer_to_remove){
                                self.dialed_peers.remove(&addr);
                                return Some(addr);
                            }

                            None
                        }).collect::<HashSet<_>>();

                        // TODO: We send a request to the discovery handler to ban the set of addresses.
                        // if let Err(send_err) = handler.sender.send(HandlerRequest::BanAddrs { addrs_to_ban: addrs_to_ban.into(), ban_duration: ban_peers.ban_duration }).await{
                        //     warn!(err = ?send_err, "Impossible to send a request to the discovery handler. The channel connection is dropped.");
                        // }
                    },
                }
            }
        });
        GossipHandle::new(enr_sender, block_tx)
    }
}
