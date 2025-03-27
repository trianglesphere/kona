//! Net Subcommand

use crate::flags::{GlobalArgs, P2PArgs, RPCArgs};
use clap::Parser;
use kona_p2p::Network;
use libp2p::multiaddr::{Multiaddr, Protocol};
use std::net::SocketAddr;

/// The `net` Subcommand
///
/// The `net` subcommand is used to run the networking stack for the `kona-node`.
///
/// # Usage
///
/// ```sh
/// kona-node net [FLAGS] [OPTIONS]
/// ```
#[derive(Parser, Debug, Clone)]
#[command(about = "Runs the networking stack for the kona-node.")]
pub struct NetCommand {
    /// P2P CLI Flags
    #[command(flatten)]
    pub p2p: P2PArgs,
    /// RPC CLI Flags
    #[command(flatten)]
    pub rpc: RPCArgs,
}

impl NetCommand {
    /// Run the Net subcommand.
    pub async fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        let signer = args.signer()?;
        tracing::info!("Gossip configured with signer: {:?}", signer);

        // TODO: RPC server setup

        let ip = self.p2p.listen_ip;
        let disc_addr = SocketAddr::new(ip, self.p2p.listen_udp_port);
        let mut gossip_addr = Multiaddr::from(ip);
        gossip_addr.push(Protocol::Tcp(self.p2p.listen_tcp_port));
        tracing::info!("Starting gossip driver on {:?}", gossip_addr);

        let mut network = Network::builder()
            .with_discovery_address(disc_addr)
            .with_chain_id(args.l2_chain_id)
            .with_gossip_address(gossip_addr)
            .with_unsafe_block_signer(signer)
            .build()?;

        let mut recv =
            network.take_unsafe_block_recv().ok_or(anyhow::anyhow!("No unsafe block receiver"))?;
        network.start()?;
        tracing::info!("Gossip driver started, receiving blocks.");
        loop {
            match recv.recv().await {
                Some(block) => {
                    tracing::info!("Received unsafe block: {:?}", block);
                }
                None => {
                    tracing::warn!("Failed to receive unsafe block");
                }
            }
        }
    }
}
