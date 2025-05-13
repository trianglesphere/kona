//! Contains the handler trait.

use libp2p::gossipsub::{Message, MessageAcceptance, TopicHash};
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;

/// Handler
///
/// A handler processes incoming messages and determines their acceptance
/// within the network. It is responsible for managing the state of seen hashes
/// and deciding whether to accept or reject messages based on their content.
/// In this way, it is expected that handler implementations will be stateful.
///
/// Handler implementations must specify the topics of interest which are used
/// to subscribe via Gossipsub.
pub trait Handler: Send {
    /// Manages validation and further processing of messages.
    ///
    /// This is a stateful method, because the handler needs to keep track of seen hashes.
    fn handle(&mut self, msg: Message) -> (MessageAcceptance, Option<OpNetworkPayloadEnvelope>);

    /// Specifies the topics of interest.
    fn topics(&self) -> Vec<TopicHash>;
}
