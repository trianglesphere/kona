//! Unified rollup binary with Kona Node ExEx integration.

#![doc(issue_tracker_base_url = "https://github.com/op-rs/kona/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(rust_2018_idioms)]

pub mod cli;
pub mod exex;
pub mod flags;
pub mod node_builder;

pub use cli::Cli;
pub use exex::KonaNodeExEx;
pub use flags::GlobalArgs;
pub use node_builder::run_op_reth_with_kona_exex;

/// Binary version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
