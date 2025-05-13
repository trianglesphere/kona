//! Contains the [`LocalNode`].

use discv5::{Enr, enr::k256};
use kona_peers::OpStackEnr;
use std::net::IpAddr;

/// The local node information exposed by the discovery service to the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNode {
    /// The keypair to use for the local node. Should match the keypair used by the
    /// gossip service.
    pub signing_key: k256::ecdsa::SigningKey,
    /// The IP address to advertise.
    pub ip: IpAddr,
    /// The TCP port to advertise.
    pub tcp_port: u16,
    /// Fallback UDP port.
    pub udp_port: u16,
}

impl LocalNode {
    /// Creates a new [`LocalNode`] instance.
    pub const fn new(
        signing_key: k256::ecdsa::SigningKey,
        ip: IpAddr,
        tcp_port: u16,
        udp_port: u16,
    ) -> Self {
        Self { signing_key, ip, tcp_port, udp_port }
    }
}

impl LocalNode {
    /// Build the local node ENR. This should contain the information we wish to
    /// broadcast to the other nodes in the network. See
    /// [the op-node implementation](https://github.com/ethereum-optimism/optimism/blob/174e55f0a1e73b49b80a561fd3fedd4fea5770c6/op-node/p2p/discovery.go#L61-L97)
    /// for the go equivalent
    pub fn build_enr(self, chain_id: u64) -> Result<Enr, discv5::enr::Error> {
        let opstack = OpStackEnr::from_chain_id(chain_id);
        let mut opstack_data = Vec::new();
        use alloy_rlp::Encodable;
        opstack.encode(&mut opstack_data);

        let mut enr_builder = Enr::builder();
        enr_builder.add_value_rlp(OpStackEnr::OP_CL_KEY, opstack_data.into());
        match self.ip {
            IpAddr::V4(addr) => {
                enr_builder.ip4(addr).tcp4(self.tcp_port).udp4(self.udp_port);
            }
            IpAddr::V6(addr) => {
                enr_builder.ip6(addr).tcp6(self.tcp_port).udp6(self.udp_port);
            }
        }

        enr_builder.build(&self.signing_key.into())
    }
}
