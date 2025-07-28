//! Verifier CLI Flags
//!
//! These flags are used to configure the verifier (validator) mode of the node.

use clap::Parser;
use kona_node_service::VerifierConfig;

/// Verifier CLI Flags
#[derive(Parser, Clone, Debug, PartialEq, Eq)]
pub struct VerifierArgs {
    /// Number of L1 blocks to keep distance from the L1 head as a verifier for picking an L1
    /// origin. This confirmation delay helps prevent instability from L1 chain reorgs.
    #[arg(long = "verifier.l1-confs", default_value = "4", env = "KONA_NODE_VERIFIER_L1_CONFS")]
    pub verifier_l1_confs: u64,
}

impl Default for VerifierArgs {
    fn default() -> Self {
        // Construct default values using the clap parser.
        // This works since none of the cli flags are required.
        Self::parse_from::<[_; 0], &str>([])
    }
}

impl VerifierArgs {
    /// Creates a [`VerifierConfig`] from the [`VerifierArgs`].
    pub const fn config(&self) -> VerifierConfig {
        VerifierConfig { l1_confirmations: self.verifier_l1_confs }
    }
}
