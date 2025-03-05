//! Trait extension module.

use alloy_primitives::{Bytes, U64};
use alloy_provider::{Network, Provider};
use alloy_rlp::Decodable;
use async_trait::async_trait;
use kona_derive::{
    errors::{PipelineError, PipelineErrorKind},
    traits::L2ChainProvider,
};
use kona_genesis::{ChainGenesis, RollupConfig, SystemConfig};
use kona_protocol::{BatchValidationProvider, L2BlockInfo, to_system_config};
use op_alloy_consensus::OpBlock;
use std::sync::Arc;

/// An error type for the `L2ChainProviderExt` trait.
#[derive(Debug, thiserror::Error)]
pub enum L2ChainProviderError {
    /// Failed to find a block.
    #[error("Failed to fetch block {0}")]
    BlockNotFound(u64),
    /// Failed to construct [L2BlockInfo] from the block and genesis.
    #[error("Failed to construct L2BlockInfo from block {0} and genesis")]
    L2BlockInfoConstruction(u64),
    /// Failed to decode an [OpBlock] from the raw block.
    #[error("Failed to decode OpBlock from raw block {0}")]
    OpBlockDecode(u64),
    /// Failed to convert the block into a [SystemConfig].
    #[error("Failed to convert block {0} into SystemConfig")]
    SystemConfigConversion(u64),
}

impl From<L2ChainProviderError> for PipelineErrorKind {
    fn from(e: L2ChainProviderError) -> Self {
        match e {
            L2ChainProviderError::BlockNotFound(_) => {
                PipelineErrorKind::Temporary(PipelineError::Provider("block not found".to_string()))
            }
            L2ChainProviderError::L2BlockInfoConstruction(_) => PipelineErrorKind::Temporary(
                PipelineError::Provider("l2 block info construction failed".to_string()),
            ),
            L2ChainProviderError::OpBlockDecode(_) => PipelineErrorKind::Temporary(
                PipelineError::Provider("op block decode failed".to_string()),
            ),
            L2ChainProviderError::SystemConfigConversion(_) => PipelineErrorKind::Temporary(
                PipelineError::Provider("system config conversion failed".to_string()),
            ),
        }
    }
}

/// An extension of the `L2ChainProvider` trait.
#[async_trait]
pub trait L2ChainProviderExt<N>: Provider<N> {}

#[async_trait]
impl<N, P> L2ChainProviderExt<N> for P
where
    P: Provider<N>,
    N: Network,
{
}

#[async_trait]
impl<N> BatchValidationProvider for L2ChainProviderExt<N>
where
    N: Network,
{
    type Error = L2ChainProviderError;

    async fn l2_block_info_by_number(
        &mut self,
        number: u64,
        genesis: &ChainGenesis,
    ) -> Result<L2BlockInfo, <Self as BatchValidationProvider>::Error> {
        let block = self
            .block_by_number(number)
            .await
            .map_err(|_| L2ChainProviderError::BlockNotFound(number))?;
        L2BlockInfo::from_block_and_genesis(&block, &genesis)
            .map_err(|_| L2ChainProviderError::L2BlockInfoConstruction(number))
    }

    async fn block_by_number(
        &mut self,
        number: u64,
    ) -> Result<OpBlock, <Self as BatchValidationProvider>::Error> {
        let raw_block: Bytes = self
            .raw_request("debug_getRawBlock".into(), [U64::from(number)])
            .await
            .map_err(|_| L2ChainProviderError::BlockNotFound(number))?;
        OpBlock::decode(&mut raw_block.as_ref())
            .map_err(|_| L2ChainProviderError::OpBlockDecode(number))
    }
}

#[async_trait]
impl<N> L2ChainProvider for L2ChainProviderExt<N>
where
    N: Network,
{
    type Error = L2ChainProviderError;

    async fn system_config_by_number(
        &mut self,
        number: u64,
        rollup_config: Arc<RollupConfig>,
    ) -> Result<SystemConfig, <Self as BatchValidationProvider>::Error> {
        let block = self
            .block_by_number(number)
            .await
            .map_err(|_| L2ChainProviderError::BlockNotFound(number))?;
        to_system_config(&block, &rollup_config)
            .map_err(|_| L2ChainProviderError::SystemConfigConversion(number))
    }
}
