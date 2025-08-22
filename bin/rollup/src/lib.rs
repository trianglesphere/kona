//! Unified rollup binary with Kona Node ExEx integration.
//!
//! This crate provides a simplified approach to running the OP Stack by embedding
//! the Kona rollup node directly into op-reth as an Execution Extension (ExEx).

#![doc(issue_tracker_base_url = "https://github.com/op-rs/kona/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(rust_2018_idioms)]

mod exex;
pub use exex::{ExExContext, KonaNodeExEx};

/// Version information for the rollup binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
