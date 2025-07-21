#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
extern crate tracing;

mod metrics;
pub use metrics::Metrics;

pub mod rpc;
pub use rpc::{
    Connectedness, Direction, GossipScores, P2pRpcRequest, PeerCount, PeerDump, PeerInfo,
    PeerScores, PeerStats, ReqRespScores, TopicScores,
};

mod behaviour;
pub use behaviour::{Behaviour, BehaviourError};

mod config;
pub use config::{
    DEFAULT_MESH_D, DEFAULT_MESH_DHI, DEFAULT_MESH_DLAZY, DEFAULT_MESH_DLO,
    GLOBAL_VALIDATE_THROTTLE, GOSSIP_HEARTBEAT, MAX_GOSSIP_SIZE, MAX_OUTBOUND_QUEUE,
    MAX_VALIDATE_QUEUE, MIN_GOSSIP_SIZE, PEER_SCORE_INSPECT_FREQUENCY, SEEN_MESSAGES_TTL,
    default_config, default_config_builder,
};

mod gate;
pub use gate::ConnectionGate; // trait

mod gater;
pub use gater::{
    ConnectionGater, // implementation
    DialInfo,
    GaterConfig,
};

mod builder;
pub use builder::GossipDriverBuilder;

mod error;
pub use error::{GossipDriverBuilderError, HandlerEncodeError, PublishError};

mod event;
pub use event::Event;

mod handler;
pub use handler::{BlockHandler, Handler};

mod driver;
pub use driver::GossipDriver;

mod block_validity;
pub use block_validity::BlockInvalidError;

#[cfg(test)]
pub(crate) use block_validity::tests::*;