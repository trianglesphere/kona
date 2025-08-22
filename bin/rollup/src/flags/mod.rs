//! CLI flags for the rollup binary.

mod globals;
pub use globals::GlobalArgs;

mod p2p;
pub use p2p::P2PArgs;

mod rpc;
pub use rpc::RpcArgs;

mod sequencer;
pub use sequencer::SequencerArgs;
