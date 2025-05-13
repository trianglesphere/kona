//! The configuration wrapper for the gossip [`libp2p::gossipsub::Config`].

use crate::Parameters;
use lazy_static::lazy_static;
use libp2p::gossipsub::{Config as GossipConfig, ConfigBuilder, Message, MessageId};
use openssl::sha::sha256;
use snap::raw::Decoder;
use std::time::Duration;

lazy_static! {
    /// The gossip heartbeat.
    pub static ref GOSSIP_HEARTBEAT: Duration = Duration::from_millis(500);

    /// The seen messages TTL.
    /// Limits the duration that message IDs are remembered for gossip deduplication purposes.
    pub static ref SEEN_MESSAGES_TTL: Duration = 130 * *GOSSIP_HEARTBEAT;

    /// The pper score inspect frequency.
    /// The frequency at which peer scores are inspected.
    pub static ref PEER_SCORE_INSPECT_FREQUENCY: Duration = 15 * Duration::from_secs(1);
}

/// Wraps the [`libp2p::gossipsub::Config`] type.
#[derive(Debug, Clone)]
pub struct Config {
    /// The inner gossip configuration.
    pub inner: GossipConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self { inner: Self::default_inner_config() }
    }
}

impl Config {
    /// Constructs the default [`libp2p::gossipsub::Config`] using the [`ConfigBuilder`].
    ///
    /// ## Panics
    ///
    /// Panics if the Config is invalid. Since values are hard-coded,
    /// this is not expected to panic.
    pub fn default_inner_config() -> GossipConfig {
        Self::default_config_builder().build().expect("default gossipsub config must be valid")
    }

    /// Builds the default gossipsub configuration.
    ///
    /// Notable defaults:
    /// - flood_publish: false (call `.flood_publish(true)` on the [ConfigBuilder] to enable)
    /// - backoff_slack: 1
    /// - heart beat interval: 1 second
    /// - peer exchange is disabled
    /// - maximum byte size for gossip messages: 2048 bytes
    ///
    /// # Returns
    ///
    /// A [`ConfigBuilder`] with the default gossipsub configuration already set.
    /// Call `.build()` on the returned builder to get the final [libp2p::gossipsub::Config].
    pub fn default_config_builder() -> ConfigBuilder {
        let mut builder = ConfigBuilder::default();
        builder
            .mesh_n(Parameters::DEFAULT_MESH_D)
            .mesh_n_low(Parameters::DEFAULT_MESH_DLO)
            .mesh_n_high(Parameters::DEFAULT_MESH_DHI)
            .gossip_lazy(Parameters::DEFAULT_MESH_DLAZY)
            .heartbeat_interval(*GOSSIP_HEARTBEAT)
            .fanout_ttl(Duration::from_secs(60))
            .history_length(12)
            .history_gossip(3)
            .flood_publish(false)
            .support_floodsub()
            .max_transmit_size(Parameters::MAX_GOSSIP_SIZE)
            .duplicate_cache_time(Duration::from_secs(120))
            .validation_mode(libp2p::gossipsub::ValidationMode::None)
            .validate_messages()
            .message_id_fn(Self::compute_message_id);

        builder
    }

    /// Computes the [`MessageId`] of a `gossipsub` [`Message`].
    pub fn compute_message_id(msg: &Message) -> MessageId {
        let mut decoder = Decoder::new();
        let id = decoder.decompress_vec(&msg.data).map_or_else(
            |_| {
                warn!(target: "cfg", "Failed to decompress message, using invalid snappy");
                let domain_invalid_snappy: Vec<u8> = vec![0x0, 0x0, 0x0, 0x0];
                sha256([domain_invalid_snappy.as_slice(), msg.data.as_slice()].concat().as_slice())
                    [..20]
                    .to_vec()
            },
            |data| {
                let domain_valid_snappy: Vec<u8> = vec![0x1, 0x0, 0x0, 0x0];
                sha256([domain_valid_snappy.as_slice(), data.as_slice()].concat().as_slice())[..20]
                    .to_vec()
            },
        );

        MessageId(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructs_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.inner.mesh_n(), Parameters::DEFAULT_MESH_D);
        assert_eq!(cfg.inner.mesh_n_low(), Parameters::DEFAULT_MESH_DLO);
        assert_eq!(cfg.inner.mesh_n_high(), Parameters::DEFAULT_MESH_DHI);
    }

    #[test]
    fn test_compute_message_id_invalid_snappy() {
        let msg = Message {
            source: None,
            data: vec![1, 2, 3, 4, 5],
            sequence_number: None,
            topic: libp2p::gossipsub::TopicHash::from_raw("test"),
        };

        let id = Config::compute_message_id(&msg);
        let hashed = sha256(&[&[0x0, 0x0, 0x0, 0x0], [1, 2, 3, 4, 5].as_slice()].concat());
        assert_eq!(id.0, hashed[..20].to_vec());
    }

    #[test]
    fn test_compute_message_id_valid_snappy() {
        let compressed = snap::raw::Encoder::new().compress_vec(&[1, 2, 3, 4, 5]).unwrap();
        let msg = Message {
            source: None,
            data: compressed,
            sequence_number: None,
            topic: libp2p::gossipsub::TopicHash::from_raw("test"),
        };

        let id = Config::compute_message_id(&msg);
        let hashed = sha256(&[&[0x1, 0x0, 0x0, 0x0], [1, 2, 3, 4, 5].as_slice()].concat());
        assert_eq!(id.0, hashed[..20].to_vec());
    }
}
