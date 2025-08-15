//! Builder for creating test derivation actors.

use std::sync::Arc;

use alloy_chains::Chain;
use alloy_consensus::{Block, EthereumTxEnvelope, TxEip1559, TxEip4844Variant};
use alloy_primitives::{Address, Bytes};
use arbitrary::Arbitrary;
use async_trait::async_trait;
use kona_derive::{
    CalldataSource, PipelineBuilder, frame, frames,
    test_utils::{TestAttributesBuilder, TestChainProvider, TestL2ChainProvider},
};
use kona_genesis::{RollupConfig, SystemConfig};
use kona_node_service::{
    DerivationActor, DerivationContext, DerivationState, NodeActor,
    PipelineBuilder as PipelineBuilderTrait,
};
use kona_protocol::{BlockInfo, DERIVATION_VERSION_0, L2BlockInfo};
use op_alloy_consensus::OpBlock;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use rand::RngCore;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::actors::{
    derivation::helpers::pipeline::TestPipeline, mocks::seed::SEED_GENERATOR_BUILDER,
};

use super::TestDerivation;

/// Builder for creating test derivation actors.
#[derive(Debug, Clone)]
pub(crate) struct TestDerivationBuilder {
    pub(crate) rollup_config: Arc<RollupConfig>,
    pub(crate) origin: BlockInfo,
}

impl Default for TestDerivationBuilder {
    fn default() -> Self {
        Self {
            rollup_config: Arc::new(RollupConfig::default()),
            origin: BlockInfo::default(),
            dap_source: CalldataSource::new(TestChainProvider::default(), Address::default()),
            builder: TestAttributesBuilder::default(),
            chain_provider: TestChainProvider::default(),
            l2_chain_provider: Default::default(),
        }
    }
}

impl TestDerivationBuilder {
    /// Creates a new test derivation builder.
    pub(crate) fn new() -> Self {
        let mut rng = rand::rng();
        let chain_id = rng.next_u64();
        let rollup_config =
            Arc::new(RollupConfig { l2_chain_id: Chain::from_id(chain_id), ..Default::default() });

        Self { rollup_config, ..Default::default() }
    }

    /// Sets the rollup configuration.
    pub(crate) fn with_rollup_config(mut self, config: RollupConfig) -> Self {
        self.rollup_config = Arc::new(config);
        self
    }

    /// Sets the origin.
    pub(crate) fn with_origin(mut self, origin: BlockInfo) -> Self {
        self.origin = origin;
        self
    }

    /// Sets the DAP source.
    pub(crate) fn with_dap_source(mut self, dap_source: CalldataSource<TestChainProvider>) -> Self {
        self.dap_source = dap_source;
        self
    }

    /// Sets the attributes builder.
    pub(crate) fn with_builder(mut self, builder: TestAttributesBuilder) -> Self {
        self.builder = builder;
        self
    }

    /// Sets the chain provider.
    pub(crate) fn with_chain_provider(mut self, chain_provider: TestChainProvider) -> Self {
        self.chain_provider = chain_provider;
        self
    }

    /// Sets the L2 chain provider.
    pub(crate) fn with_l2_chain_provider(mut self, l2_chain_provider: TestL2ChainProvider) -> Self {
        self.l2_chain_provider = l2_chain_provider;
        self
    }

    /// Generates n rounds of valid derivation data.
    pub fn valid_rounds(mut self, rounds: u64) -> anyhow::Result<Self> {
        let mut seed_generator = SEED_GENERATOR_BUILDER.next_generator();

        let randomness_source = seed_generator.random_bytes(1024);
        let mut u = arbitrary::Unstructured::new(&randomness_source);
        let batch_inbox_address: Address = Arbitrary::arbitrary(&mut u)?;

        for i in 0..rounds {
            let randomness_source = seed_generator.random_bytes(1024);
            let mut u = arbitrary::Unstructured::new(&randomness_source);

            let attributes = OpPayloadAttributes {
                payload_attributes: Arbitrary::arbitrary(&mut u)?,
                transactions: Arbitrary::arbitrary(&mut u)?,
                no_tx_pool: None,
                gas_limit: None,
                eip_1559_params: None,
            };

            self.builder.attributes.push(Ok(attributes));

            let randomness_source = seed_generator.random_bytes(1024);
            let mut u = arbitrary::Unstructured::new(&randomness_source);

            let mut block = Block::<TxEip1559>::arbitrary(&mut u)?;
            block.header.number = i;
            let block_info = block.clone().into();

            let mut txs: Vec<EthereumTxEnvelope<TxEip4844Variant>> = Arbitrary::arbitrary(&mut u)?;
            for tx in txs.iter_mut() {
                *tx = tx.clone().map_eip4844(|mut tx| {
                    tx.as_mut().to = batch_inbox_address;
                    tx
                });
            }

            self.chain_provider.insert_block_with_transactions(i, block_info, txs);
            self.chain_provider.insert_header(block_info.hash, block.header.clone());
            self.chain_provider.insert_receipts(block_info.hash, Arbitrary::arbitrary(&mut u)?);

            let randomness_source = seed_generator.random_bytes(1024);
            let mut u = arbitrary::Unstructured::new(&randomness_source);

            let mut block: L2BlockInfo = Arbitrary::arbitrary(&mut u)?;
            block.block_info.number = i;
            block.l1_origin.number = i;

            let mut op_block: OpBlock = Arbitrary::arbitrary(&mut u)?;
            op_block.header.number = i;
            self.l2_chain_provider.blocks.push(block);
            self.l2_chain_provider.op_blocks.push(op_block);

            let randomness_source = seed_generator.random_bytes(1024);
            let mut u = arbitrary::Unstructured::new(&randomness_source);

            for i in 0..rounds {
                self.l2_chain_provider.system_configs.insert(i, SystemConfig::arbitrary(&mut u)?);
            }

            self.dap_source = CalldataSource::new(self.chain_provider.clone(), batch_inbox_address);
        }

        Ok(self)
    }

    /// Builds the test derivation actor.
    pub(crate) fn build(&self) -> TestDerivation {
        let builder = self.clone();
        let (inbound, actor) = DerivationActor::new(builder);

        let (attributes_tx, attributes_rx) = mpsc::channel(1024);
        let (reset_tx, reset_rx) = mpsc::channel(16);
        let cancellation = CancellationToken::new();

        let context = DerivationContext {
            cancellation: cancellation.clone(),
            derived_attributes_tx: attributes_tx,
            reset_request_tx: reset_tx,
        };

        // We spawn the actor in a separate task
        let handle = tokio::spawn(async move { actor.start(context).await });

        TestDerivation { inbound: inbound.into(), attributes_rx, reset_rx, handle, cancellation }
    }
}

#[async_trait]
impl PipelineBuilderTrait for TestPipeline {
    type Pipeline = TestPipeline;

    async fn build(self) -> DerivationState<Self::Pipeline> {
        DerivationState::new(self)
    }
}
