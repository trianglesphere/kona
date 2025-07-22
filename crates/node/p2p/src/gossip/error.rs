//! Contains the error from the gossip builder.

use crate::BehaviourError;
use derive_more::From;
use libp2p::{Multiaddr, PeerId};
use std::net::IpAddr;
use thiserror::Error;

/// An error publishing a payload.
#[derive(Debug, Error)]
pub enum PublishError {
    /// An error occurred publishing the payload.
    #[error("Failed to publish payload: {0}")]
    PublishError(#[from] libp2p::gossipsub::PublishError),
    /// An error occurred when encoding the payload.
    #[error("Failed to encode payload: {0}")]
    EncodeError(#[from] HandlerEncodeError),
}

/// An error occurred when encoding the payload from the block handler.
#[derive(Debug, Error)]
pub enum HandlerEncodeError {
    /// Failed to encode the payload envelope.
    #[error("Failed to encode payload: {0}")]
    PayloadEncodeError(#[from] op_alloy_rpc_types_engine::PayloadEnvelopeEncodeError),
    /// The topic is unknown.
    #[error("Unknown topic: {0}")]
    UnknownTopic(libp2p::gossipsub::TopicHash),
}

/// An error representing why a peer cannot be dialed.
#[derive(Debug, Clone, Error)]
pub enum DialError {
    /// Failed to extract PeerId from Multiaddr.
    #[error("Failed to extract PeerId from Multiaddr: {addr}")]
    InvalidMultiaddr {
        /// The multiaddr that failed to parse.
        addr: Multiaddr,
    },
    /// Peer is already being dialed.
    #[error("Already dialing peer: {peer_id}")]
    AlreadyDialing {
        /// The peer ID that is already being dialed.
        peer_id: PeerId,
    },
    /// Dial threshold reached for this peer.
    #[error("Dial threshold reached for peer: {peer_id}")]
    ThresholdReached {
        /// The peer ID that reached the dial threshold.
        peer_id: PeerId,
    },
    /// Peer is blocked.
    #[error("Peer is blocked: {peer_id}")]
    BlockedPeer {
        /// The blocked peer ID.
        peer_id: PeerId,
    },
    /// Failed to extract IP address from Multiaddr.
    #[error("Failed to extract IP address from Multiaddr: {addr}")]
    InvalidIpAddr {
        /// The multiaddr that failed to parse.
        addr: Multiaddr,
    },
    /// IP address is blocked.
    #[error("IP address is blocked: {ip_addr}")]
    BlockedAddress {
        /// The blocked IP address.
        ip_addr: IpAddr,
    },
    /// IP address is in a blocked subnet.
    #[error("IP address is in a blocked subnet: {ip_addr}")]
    BlockedSubnet {
        /// The IP address in a blocked subnet.
        ip_addr: IpAddr,
    },
}

/// An error type for the [`crate::GossipDriverBuilder`].
#[derive(Debug, Clone, PartialEq, Eq, From, Error)]
pub enum GossipDriverBuilderError {
    /// A TCP error.
    #[error("TCP error")]
    TcpError,
    /// An error when setting the behaviour on the swarm builder.
    #[error("error setting behaviour on swarm builder")]
    WithBehaviourError,
    /// An error when building the gossip behaviour.
    #[error("error building gossip behaviour")]
    BehaviourError(BehaviourError),
    /// An error when setting up the sync request/response protocol.
    #[error("error setting up sync request/response protocol")]
    SetupSyncReqRespError,
    /// The sync request/response protocol has already been accepted.
    #[error("sync request/response protocol already accepted")]
    SyncReqRespAlreadyAccepted,
}
