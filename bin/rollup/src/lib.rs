//! Unified rollup binary with Kona Node ExEx integration.

#![doc(issue_tracker_base_url = "https://github.com/op-rs/kona/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(rust_2018_idioms)]

mod cli;
mod exex;

pub use cli::RollupCli;
pub use exex::KonaNodeExEx;

/// Binary version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
