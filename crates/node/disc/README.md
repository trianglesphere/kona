# `kona-disc`

Peer discovery library for the OP Stack using discv5.

This crate provides peer discovery functionality for the Kona rollup node, allowing nodes to discover and connect to other peers in the network through the discv5 protocol.

## Features

- **Discovery Service**: Discover peers using the discv5 protocol
- **ENR Management**: Handle Ethereum Node Records for peer identification  
- **Metrics**: Optionally track discovery metrics with the `metrics` feature

## Usage

```rust
use kona_disc::{Discv5Builder, LocalNode};
use discv5::enr::CombinedKey;
use std::net::{IpAddr, Ipv4Addr};

let CombinedKey::Secp256k1(secret_key) = CombinedKey::generate_secp256k1() else {
    unreachable!()
};

let socket = LocalNode::new(
    secret_key,
    IpAddr::V4(Ipv4Addr::UNSPECIFIED),
    9099,
    9099,
);

let discovery_builder = Discv5Builder::new(
    socket,
    10, // chain_id
    discv5::ConfigBuilder::new(discv5::ListenConfig::Ipv4 {
        ip: Ipv4Addr::UNSPECIFIED,
        port: 9099,
    })
    .build(),
);

let mut discovery = discovery_builder.build()?;
let (handler, enr_receiver) = discovery.start();
```

## Features

- `metrics` - Enable metrics collection for discovery operations