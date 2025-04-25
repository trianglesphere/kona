//! Contains the [`RuntimeLoader`] implementation.

use crate::RuntimeCall;
use kona_genesis::RollupConfig;
use kona_providers_alloy::AlloyChainProvider;
use std::sync::Arc;
use url::Url;

/// The runtime loader.
#[derive(Debug, Clone)]
pub struct RuntimeLoader {
    /// The L1 Client
    pub provider: AlloyChainProvider,
    /// The rollup config.
    pub config: Arc<RollupConfig>,
}

impl RuntimeLoader {
    /// Constructs a new [`RuntimeLoader`] with the given provider [`Url`].
    pub fn new(l1_eth_rpc: Url, config: Arc<RollupConfig>) -> Self {
        let provider = AlloyChainProvider::new_http(l1_eth_rpc, 100);
        Self { provider, config }
    }

    /// Creates a new [`RuntimeCall`] that can be configured with a [`kona_protocol::BlockInfo`].
    ///
    /// By default, `.await` will load the latest block.
    /// Chain a `.block_info(block_info)` to load a specific block.
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut loader = RuntimeLoader::new(url, config);
    /// let block_info = loader.provider.block_info_by_number(1).await?;
    /// let runtime_config = loader.load().block_info(block_info).await?;
    /// ```
    pub fn load(&mut self) -> RuntimeCall {
        RuntimeCall::new(self.provider.clone(), self.config.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    use kona_rpc::ProtocolVersionFormatV0;

    const RPC_URL: &str = "https://docs-demo.quiknode.pro/";

    #[tokio::test]
    async fn test_online_runtime_loader() {
        kona_cli::init_test_tracing();

        // Load the OP Mainnet config.
        let chain_id = kona_genesis::OP_MAINNET_CHAIN_ID;
        let config =
            kona_registry::ROLLUP_CONFIGS.get(&chain_id).expect("Invalid chain ID").clone();

        // Construct the runtime loader.
        let config = Arc::new(config);
        let url = Url::parse(RPC_URL).unwrap();
        let mut loader = RuntimeLoader::new(url.clone(), config);

        // Load the runtime config.
        let version = ProtocolVersionFormatV0 { major: 9, ..Default::default() };
        let expected = RuntimeConfig {
            unsafe_block_signer_address: address!("aaaa45d9549eda09e70937013520214382ffc4a2"),
            required_protocol_version: ProtocolVersion::V0(version),
            recommended_protocol_version: ProtocolVersion::V0(version),
        };
        let runtime_config = loader.load().await.unwrap();
        assert_eq!(runtime_config, expected);
    }
}
