//! Verifier CLI Flags
//!
//! These are based on verifier flags from the [`op-node`][op-node] CLI.
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/flags/flags.go

use clap::Parser;

/// Verifier CLI Flags
#[derive(Parser, Clone, Debug, PartialEq, Eq)]
pub struct VerifierArgs {
    /// Number of L1 blocks to keep distance from the L1 head for picking an L1 origin as a
    /// verifier.
    #[arg(long = "verifier.l1-confs", default_value = "4", env = "KONA_NODE_VERIFIER_L1_CONFS")]
    pub l1_confs: u64,
}

impl Default for VerifierArgs {
    fn default() -> Self {
        // Construct default values using the clap parser.
        // This works since none of the cli flags are required.
        Self::parse_from::<[_; 0], &str>([])
    }
}