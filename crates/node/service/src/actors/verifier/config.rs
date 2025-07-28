//! Configuration for the verifier mode.

/// Configuration for the verifier (validator) mode.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct VerifierConfig {
    /// Number of L1 confirmations required before blocks are considered valid.
    /// This creates a confirmation delay to prevent instability from L1 chain reorgs.
    pub l1_confirmations: u64,
}
