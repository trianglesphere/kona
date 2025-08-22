#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

//! # Kona Local Providers
//!
//! This crate provides local buffered provider implementations for caching and reorg handling.
//!
//! ## Features
//!
//! - **BufferedL2Provider**: A provider that wraps an underlying provider with caching and reorg
//!   handling
//! - **ChainStateBuffer**: LRU cache for managing chain state with reorg support
//! - **Chain Event Handling**: Support for processing ExEx notifications for chain events
//!
//! ## Usage
//!
//! ```rust,no_run
//! use kona_providers_alloy::AlloyL2ChainProvider;
//! use kona_providers_local::{BufferedL2Provider, ChainStateEvent};
//! use std::sync::Arc;
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create an underlying provider
//!     let url = "http://localhost:8545".parse()?;
//!     let alloy_provider = alloy_provider::RootProvider::new_http(url);
//!     let rollup_config = Arc::new(Default::default());
//!     let inner = AlloyL2ChainProvider::new(alloy_provider, rollup_config, 100);
//!
//!     // Wrap with buffered provider
//!     let provider = BufferedL2Provider::new(inner, 1000, 64);
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
