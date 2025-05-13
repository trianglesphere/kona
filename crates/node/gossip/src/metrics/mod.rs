//! Metrics for the gossip layer.

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the gauge that tracks gossip events.
    pub const GOSSIP_EVENT: &str = "kona_node_gossip_events";

    /// Identifier for the gauge that tracks libp2p gossipsub connections.
    pub const GOSSIPSUB_CONNECTION: &str = "kona_node_gossipsub_connection";

    /// Identifier for the gauge that tracks unsafe blocks published.
    pub const UNSAFE_BLOCK_PUBLISHED: &str = "kona_node_unsafe_block_published";

    /// Identifier for the gauge that tracks the number of connected peers.
    pub const GOSSIP_PEER_COUNT: &str = "kona_node_swarm_peer_count";

    /// Identifier for the gauge that tracks the number of dialed peers.
    pub const DIAL_PEER: &str = "kona_node_dial_peer";

    /// Timer for the time taken to store ENRs in the bootstore.
    pub const ENR_STORE_TIME: &str = "kona_node_enr_store_time";

    /// Initializes metrics for the gossip layer.
    ///
    /// This does two things:
    /// * Describes various metrics.
    /// * Initializes metrics to 0 so they can be queried immediately.
    #[cfg(feature = "metrics")]
    pub fn init() {
        Self::describe();
        Self::zero();
    }

    /// Describes metrics used in [`kona_gossip`][crate].
    #[cfg(feature = "metrics")]
    pub fn describe() {
        metrics::describe_gauge!(Self::GOSSIP_EVENT, "Gossip events received by the libp2p Swarm");
        metrics::describe_gauge!(Self::DIAL_PEER, "Number of peers dialed by the libp2p Swarm");
        metrics::describe_gauge!(
            Self::UNSAFE_BLOCK_PUBLISHED,
            "Number of OpNetworkPayloadEnvelope gossipped out through the libp2p Swarm"
        );
        metrics::describe_histogram!(
            Self::ENR_STORE_TIME,
            "Observations of elapsed time to store ENRs in the on-disk bootstore"
        );
        metrics::describe_gauge!(
            Self::GOSSIP_PEER_COUNT,
            "Number of peers connected to the libp2p gossip Swarm"
        );
        metrics::describe_gauge!(
            Self::GOSSIPSUB_CONNECTION,
            "Connections made to the libp2p Swarm"
        );
    }

    /// Initializes metrics to `0` so they can be queried immediately by consumers of prometheus
    /// metrics.
    #[cfg(feature = "metrics")]
    pub fn zero() {
        // Gossip Events
        kona_macros::set!(gauge, Self::GOSSIP_EVENT, "total", "total", 0);
        kona_macros::set!(gauge, Self::GOSSIP_EVENT, "not_supported", "not_supported", 0);

        // Peer dials
        kona_macros::set!(gauge, Self::DIAL_PEER, 0);

        // Unsafe Blocks
        kona_macros::set!(gauge, Self::UNSAFE_BLOCK_PUBLISHED, 0);

        // Peer Counts
        kona_macros::set!(gauge, Self::GOSSIP_PEER_COUNT, 0);

        // Connection
        kona_macros::set!(gauge, Self::GOSSIPSUB_CONNECTION, "type", "connected", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_CONNECTION, "type", "outgoing_error", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_CONNECTION, "type", "incoming_error", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_CONNECTION, "type", "closed", 0);
    }
}
