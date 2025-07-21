# `kona-gossip`

Gossip networking library for the OP Stack using libp2p.

This crate provides gossip functionality for the Kona rollup node, allowing nodes to participate in the peer-to-peer gossip network for block propagation and other protocol messages.

## Features

- **Gossip Protocol**: Participate in libp2p gossipsub for block propagation
- **Connection Management**: Handle peer connections and scoring
- **Block Validation**: Validate blocks received through gossip
- **RPC Interface**: P2P RPC types and functionality for network administration
- **Metrics**: Optionally track gossip metrics with the `metrics` feature

## Usage

```rust
use kona_gossip::{GossipDriverBuilder, default_config};
use libp2p::{Multiaddr, identity::Keypair};

let gossip_config = default_config();
let keypair = Keypair::generate_secp256k1();
let address = Multiaddr::from(std::net::Ipv4Addr::UNSPECIFIED);

let builder = GossipDriverBuilder::new()
    .with_config(gossip_config)
    .with_keypair(keypair)
    .with_listen_address(address);

let driver = builder.build()?;
```

## RPC API

The crate provides RPC types compatible with the op-node P2P API for network administration:

- Peer management (connect, disconnect, block peers)
- Network statistics and metrics
- Discovery table access

## Features

- `metrics` - Enable metrics collection for gossip operations