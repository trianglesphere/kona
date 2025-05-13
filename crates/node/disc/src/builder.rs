//! Contains a builder for the discovery service.

use discv5::{Config, Discv5, Enr};
use std::path::PathBuf;
use tokio::time::Duration;

use crate::{BuildError, Discv5Driver, LocalNode};

/// Discovery service builder.
#[derive(Debug, Default, Clone)]
pub struct Discv5Builder {
    /// The node information advertised by the discovery service.
    local_node: Option<LocalNode>,
    /// The chain ID of the network.
    chain_id: Option<u64>,
    /// The interval to find peers.
    interval: Option<Duration>,
    /// The interval to randomize discovery peers.
    randomize: Option<Duration>,
    /// The discovery config for the discovery service.
    discovery_config: Option<Config>,
    /// An optional path to the bootstore.
    bootstore: Option<PathBuf>,
    /// Additional bootnodes to manually add to the initial bootstore
    bootnodes: Vec<Enr>,
    /// The interval to store the bootnodes to disk.
    store_interval: Option<Duration>,
    /// Whether or not to forward the initial set of valid ENRs to the gossip layer.
    forward: bool,
}

impl From<crate::Config> for Discv5Builder {
    fn from(config: crate::Config) -> Self {
        let mut builder = Self::new();
        if let Some(store) = config.bootstore {
            builder = builder.with_bootstore(store);
        }
        builder
            .with_bootnodes(config.bootnodes)
            .with_local_node(config.discovery_address)
            .with_chain_id(config.l2_chain_id)
            .with_interval(config.discovery_interval)
            .with_discovery_config(config.discovery_config)
            .with_discovery_randomice(config.discovery_randomize)
    }
}

impl Discv5Builder {
    /// Creates a new [`Discv5Builder`] instance.
    pub const fn new() -> Self {
        Self {
            local_node: None,
            chain_id: None,
            interval: None,
            discovery_config: None,
            randomize: None,
            bootstore: None,
            bootnodes: Vec::new(),
            store_interval: None,
            forward: true,
        }
    }

    /// Sets the bootstore path.
    pub fn with_bootstore(mut self, bootstore: PathBuf) -> Self {
        self.bootstore = Some(bootstore);
        self
    }

    /// Sets the initial bootnodes to add to the bootstore.
    pub fn with_bootnodes(mut self, bootnodes: Vec<Enr>) -> Self {
        self.bootnodes = bootnodes;
        self
    }

    /// Sets the interval to store the bootnodes to disk.
    pub const fn with_store_interval(mut self, store_interval: Duration) -> Self {
        self.store_interval = Some(store_interval);
        self
    }

    /// Sets the discovery service advertised local node information.
    pub fn with_local_node(mut self, local_node: LocalNode) -> Self {
        self.local_node = Some(local_node);
        self
    }

    /// Sets the chain ID of the network.
    pub const fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Sets the interval to find peers.
    pub const fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    /// Sets the discovery config for the discovery service.
    pub fn with_discovery_config(mut self, config: Config) -> Self {
        self.discovery_config = Some(config);
        self
    }

    /// Sets the interval to randomize discovery peers.
    pub const fn with_discovery_randomize(mut self, interval: Option<Duration>) -> Self {
        self.randomize = interval;
        self
    }

    /// Disables forwarding of the initial set of valid ENRs to the gossip layer.
    pub const fn disable_forward(mut self) -> Self {
        self.forward = false;
        self
    }

    /// Builds a [`Discv5Driver`].
    pub fn build(self) -> Result<Discv5Driver, BuildError> {
        let chain_id = self.chain_id.ok_or(BuildError::ChainIdNotSet)?;
        let config = self.discovery_config.ok_or(BuildError::DiscoveryConfigNotSet)?;
        let local_node = self.local_node.ok_or(BuildError::LocalNodeNotSet)?;
        let key = local_node.signing_key.clone();
        let enr = local_node.build_enr(chain_id).map_err(|_| BuildError::EnrBuildFailed)?;
        let interval = self.interval.unwrap_or(Duration::from_secs(5));
        let disc =
            Discv5::new(enr, key.into(), config).map_err(|_| BuildError::Discv5CreationFailed)?;
        let mut driver =
            Discv5Driver::new(disc, interval, chain_id, self.bootstore.clone(), self.bootnodes);
        driver.store_interval = self.store_interval.unwrap_or(Duration::from_secs(60));
        driver.forward = self.forward;
        driver.remove_interval = self.randomize;
        Ok(driver)
    }
}

#[cfg(test)]
mod tests {
    use discv5::{ConfigBuilder, ListenConfig, enr::CombinedKey};

    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_builds_valid_enr() {
        let CombinedKey::Secp256k1(k256_key) = CombinedKey::generate_secp256k1() else {
            unreachable!()
        };

        let addr = LocalNode::new(k256_key, IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9099, 9099);
        let mut builder = Discv5Builder::new();
        builder = builder.with_local_node(addr);
        builder = builder.with_chain_id(10);
        builder = builder.with_discovery_config(
            ConfigBuilder::new(ListenConfig::Ipv4 { ip: Ipv4Addr::UNSPECIFIED, port: 9099 })
                .build(),
        );
        let driver = builder.build().unwrap();
        let enr = driver.disc.local_enr();
        assert!(kona_peers::EnrValidation::validate(&enr, 10).is_valid());
    }
}
