#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

//! # Kona Local Providers
//!
//! This crate provides a pure in-memory L2 provider implementation for the Kona OP Stack.
//!
//! ## Features
//!
//! - **BufferedL2Provider**: A pure in-memory L2 provider that serves data from cached blocks
//! - **ChainStateBuffer**: LRU cache for managing chain state with reorg support
//! - **Chain Event Handling**: Support for processing execution extension notifications
//! - **No External Dependencies**: Operates entirely from in-memory state without RPC calls
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kona_providers_local::{BufferedL2Provider, ChainStateEvent};
//! use kona_genesis::RollupConfig;
//! use kona_protocol::L2BlockInfo;
//! use op_alloy_consensus::OpBlock;
//! use std::sync::Arc;
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a buffered provider with rollup configuration
//!     let rollup_config = Arc::new(RollupConfig::default());
//!     let provider = BufferedL2Provider::new(rollup_config, 1000, 64);
//!
//!     // Add blocks to the provider
//!     let block: OpBlock = unimplemented!(); // obtain from execution extension
//!     let l2_info: L2BlockInfo = unimplemented!(); // derive from block
//!     provider.add_block(block, l2_info).await?;
//!
//!     // Handle chain events
//!     let event = ChainStateEvent::ChainCommitted {
//!         new_head: alloy_primitives::B256::ZERO,
//!         committed: vec![],
//!     };
//!     provider.handle_chain_event(event).await?;
//!
//!     Ok(())
//! }
//! ```
mod buffer;
pub use buffer::{CacheStats, CachedBlock, ChainBufferError, ChainStateBuffer, ChainStateEvent};

mod buffered;
pub use buffered::{BufferedL2Provider, BufferedProviderError};

#[cfg(feature = "metrics")]
mod metrics;
#[cfg(feature = "metrics")]
pub use metrics::Metrics;
