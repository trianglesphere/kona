//! Metrics for the providers-alloy crate.

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the gauge that tracks RPC calls.
    pub const RPC_CALLS: &str = "kona_providers_alloy_rpc_calls";

    /// Initializes metrics for the providers-alloy crate.
    ///
    /// This does two things:
    /// * Describes various metrics.
    /// * Initializes metrics to 0 so they can be queried immediately.
    #[cfg(feature = "metrics")]
    pub fn init() {
        Self::describe();
        Self::zero();
    }

    /// Describes metrics used in [`kona_providers_alloy`][crate].
    #[cfg(feature = "metrics")]
    pub fn describe() {
        metrics::describe_gauge!(Self::RPC_CALLS, "RPC calls made by providers-alloy");
    }

    /// Initializes metrics to `0` so they can be queried immediately by consumers of prometheus
    /// metrics.
    #[cfg(feature = "metrics")]
    pub fn zero() {
        // Chain Provider RPC Calls
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "chain_get_block_number", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "chain_get_chain_id", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "chain_get_block_by_hash", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "chain_get_block_by_number", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "chain_get_block_receipts", 0);

        // L2 Chain Provider RPC Calls
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "l2_chain_get_block_number", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "l2_chain_get_chain_id", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "l2_chain_get_block_by_hash", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "l2_chain_get_block_by_number", 0);

        // Beacon Client RPC Calls
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "beacon_config_spec", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "beacon_genesis", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "beacon_blob_side_cars", 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "metrics")]
    fn test_metrics_init() {
        // This test verifies that metrics initialization doesn't panic
        // and that the constants are properly defined
        assert_eq!(Metrics::RPC_CALLS, "kona_providers_alloy_rpc_calls");
        
        // Test that init() can be called without panicking
        Metrics::init();
    }

    #[test]
    fn test_metrics_constants() {
        // This test works even without the metrics feature
        assert_eq!(Metrics::RPC_CALLS, "kona_providers_alloy_rpc_calls");
    }
}