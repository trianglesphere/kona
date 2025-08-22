//! Unified rollup binary with CLI and service orchestration.

#![doc(issue_tracker_base_url = "https://github.com/op-rs/kona/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(rust_2018_idioms)]

pub mod cli;
pub mod config;
pub mod error;

pub use cli::{Cli, Commands, GlobalArgs, NodeCommand};
pub use config::RollupConfig;
pub use error::{RollupError, RollupResult};

/// Version information for the rollup binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");