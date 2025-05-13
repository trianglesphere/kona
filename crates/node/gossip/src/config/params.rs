//! Gossip Configuration Parameters.

/// Container for gossip configuration parameters.
#[derive(Debug, Copy, PartialEq, Eq, Clone)]
pub struct Parameters;

impl Parameters {
    ////////////////////////////////////////////////////////////////////////////////////////////////
    // GossipSub Constants
    ////////////////////////////////////////////////////////////////////////////////////////////////

    /// The maximum gossip size.
    /// Limits the total size of gossip RPC containers as well as decompressed individual messages.
    pub const MAX_GOSSIP_SIZE: usize = 10 * (1 << 20);

    /// The minimum gossip size.
    /// Used to make sure that there is at least some data to validate the signature against.
    pub const MIN_GOSSIP_SIZE: usize = 66;

    /// The maximum outbound queue.
    pub const MAX_OUTBOUND_QUEUE: usize = 256;

    /// The maximum validate queue.
    pub const MAX_VALIDATE_QUEUE: usize = 256;

    /// The global validate throttle.
    pub const GLOBAL_VALIDATE_THROTTLE: usize = 512;

    /// The default mesh D.
    pub const DEFAULT_MESH_D: usize = 8;

    /// The default mesh D low.
    pub const DEFAULT_MESH_DLO: usize = 6;

    /// The default mesh D high.
    pub const DEFAULT_MESH_DHI: usize = 12;

    /// The default mesh D lazy.
    pub const DEFAULT_MESH_DLAZY: usize = 6;
}
