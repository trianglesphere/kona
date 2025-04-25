//! Calling module

use alloy_primitives::{Address, B256, b256};
use alloy_provider::Provider;
use futures::FutureExt;
use kona_derive::traits::ChainProvider;
use kona_genesis::RollupConfig;
use kona_protocol::BlockInfo;
use kona_providers_alloy::AlloyChainProvider;
use kona_rpc::ProtocolVersion;
use std::{future::Future, pin::Pin, sync::Arc, task::{Context, Poll}};
use futures::future::BoxFuture;

use crate::{RuntimeCallError, RuntimeConfig};

/// The storage slot that the unsafe block signer address is stored at.
const UNSAFE_BLOCK_SIGNER_ADDRESS_STORAGE_SLOT: B256 =
    b256!("0x65a7ed542fb37fe237fdfbdd70b31598523fe5b32879e307bae27a0bd9581c08");

/// The storage slot that the required protocol version is stored at.
const REQUIRED_PROTOCOL_VERSION_STORAGE_SLOT: B256 =
    b256!("0x4aaefe95bd84fd3f32700cf3b7566bc944b73138e41958b5785826df2aecace0");

/// The storage slot that the recommended protocol version is stored at.
const RECOMMENDED_PROTOCOL_VERSION_STORAGE_SLOT: B256 =
    b256!("0xe314dfc40f0025322aacc0ba8ef420b62fb3b702cf01e0cdf3d829117ac2ff1a");

/// A future that represents a runtime loader call.
#[derive(Debug)]
pub struct RuntimeCall {
    /// The L1 provider
    provider: AlloyChainProvider,
    /// The rollup config
    config: Arc<RollupConfig>,
    /// Optional block info to use for loading
    block_info: Option<BlockInfo>,
    /// The current state of the loading operation
    state: LoadState,
}

/// The internal state of the loading operation
enum LoadState {
    /// Initial state
    Init,
    /// Getting latest block number
    GettingLatestBlock(BoxFuture<'static, Result<u64, AlloyChainProviderError>>),
    /// Getting block info
    GettingBlockInfo(BoxFuture<'static, Result<BlockInfo, AlloyChainProviderError>>),
    /// Getting block hash verification
    VerifyingBlockHash(BlockInfo, BoxFuture<'static, Result<Option<Block>, AlloyTransportError>>),
    /// Getting unsafe block signer
    GettingUnsafeBlockSigner(BlockInfo, BoxFuture<'static, Result<B256, AlloyTransportError>>),
    /// Getting required protocol version
    GettingRequiredProtocolVersion(BlockInfo, Address, BoxFuture<'static, Result<B256, AlloyTransportError>>),
    /// Getting recommended protocol version
    GettingRecommendedProtocolVersion(BlockInfo, Address, ProtocolVersion, BoxFuture<'static, Result<B256, AlloyTransportError>>),
    /// Done loading
    Done,
}

impl RuntimeCall {
    /// Creates a new RuntimeLoaderCall
    pub fn new(provider: AlloyChainProvider, config: Arc<RollupConfig>) -> Self {
        Self { provider, config, block_info: None, state: LoadState::Init }
    }

    /// Sets the block info to use for loading
    pub fn block_info(mut self, block_info: BlockInfo) -> Self {
        self.block_info = Some(block_info);
        self
    }
}

impl Future for RuntimeCall {
    type Output = Result<RuntimeConfig, RuntimeCallError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match &mut self.state {
                LoadState::Init => {
                    // If we have block info, skip to verification
                    if let Some(block_info) = self.block_info.take() {
                        let fut = self.provider.inner.get_block(block_info.hash.into());
                        self.state = LoadState::VerifyingBlockHash(block_info, Box::pin(fut));
                    } else {
                        let fut = self.provider.latest_block_number();
                        self.state = LoadState::GettingLatestBlock(Box::pin(fut));
                    }
                }

                LoadState::GettingLatestBlock(fut) => {
                    match ready!(fut.as_mut().poll(cx)) {
                        Ok(block_num) => {
                            let fut = self.provider.block_info_by_number(block_num);
                            self.state = LoadState::GettingBlockInfo(Box::pin(fut));
                        }
                        Err(e) => return Poll::Ready(Err(e.into())),
                    }
                }

                LoadState::GettingBlockInfo(fut) => {
                    match ready!(fut.as_mut().poll(cx)) {
                        Ok(block_info) => {
                            let fut = self.provider.inner.get_block(block_info.hash.into());
                            self.state = LoadState::VerifyingBlockHash(block_info, Box::pin(fut));
                        }
                        Err(e) => return Poll::Ready(Err(e.into())),
                    }
                }

                LoadState::VerifyingBlockHash(block_info, fut) => {
                    match ready!(fut.as_mut().poll(cx)) {
                        Ok(Some(block)) => {
                            if block.header.hash != block_info.hash {
                                return Poll::Ready(Err(RuntimeCallError::BlockHashMismatch));
                            }
                            let fut = self.provider.inner.get_storage_at(
                                self.config.l1_system_config_address,
                                UNSAFE_BLOCK_SIGNER_ADDRESS_STORAGE_SLOT.into(),
                            ).hash(block_info.hash);
                            self.state = LoadState::GettingUnsafeBlockSigner(*block_info, Box::pin(fut));
                        }
                        Ok(None) => return Poll::Ready(Err(RuntimeLoaderError::BlockNotFound)),
                        Err(e) => return Poll::Ready(Err(e.into())),
                    }
                }

                LoadState::GettingUnsafeBlockSigner(block_info, fut) => {
                    match ready!(fut.as_mut().poll(cx)) {
                        Ok(storage) => {
                            let unsafe_block_signer = Address::from_slice(&storage.to_be_bytes_vec()[12..]);
                            if self.config.protocol_versions_address == Address::ZERO {
                                // If protocol versions address is not set, return default config
                                let config = RuntimeConfig {
                                    unsafe_block_signer_address: unsafe_block_signer,
                                    required_protocol_version: ProtocolVersion::V0(
                                        Default::default(),
                                    ),
                                    recommended_protocol_version: ProtocolVersion::V0(
                                        Default::default(),
                                    ),
                                };
                                return Poll::Ready(Ok(config));
                            }
                            let fut = self.provider.inner.get_storage_at(
                                self.config.protocol_versions_address,
                                REQUIRED_PROTOCOL_VERSION_STORAGE_SLOT.into(),
                            ).hash(block_info.hash);
                            self.state = LoadState::GettingRequiredProtocolVersion(
                                *block_info,
                                unsafe_block_signer,
                                Box::pin(fut),
                            );
                        }
                        Err(e) => return Poll::Ready(Err(e.into())),
                    }
                }

                LoadState::GettingRequiredProtocolVersion(block_info, unsafe_block_signer, fut) => {
                    match ready!(fut.as_mut().poll(cx)) {
                        Ok(storage) => {
                            match ProtocolVersion::decode(storage.into()) {
                                Ok(required_version) => {
                                    let fut = self.provider.inner.get_storage_at(
                                        self.config.protocol_versions_address,
                                        RECOMMENDED_PROTOCOL_VERSION_STORAGE_SLOT.into(),
                                    ).hash(block_info.hash);
                                    self.state = LoadState::GettingRecommendedProtocolVersion(
                                        *block_info,
                                        *unsafe_block_signer,
                                        required_version,
                                        Box::pin(fut),
                                    );
                                }
                                Err(e) => return Poll::Ready(Err(e.into())),
                            }
                        }
                        Err(e) => return Poll::Ready(Err(e.into())),
                    }
                }

                LoadState::GettingRecommendedProtocolVersion(block_info, unsafe_block_signer, required_version, fut) => {
                    match ready!(fut.as_mut().poll(cx)) {
                        Ok(storage) => {
                            match ProtocolVersion::decode(storage.into()) {
                                Ok(recommended_version) => {
                                    let config = RuntimeConfig {
                                        unsafe_block_signer_address: *unsafe_block_signer,
                                        required_protocol_version: *required_version,
                                        recommended_protocol_version: recommended_version,
                                    };
                                    self.state = LoadState::Done;
                                    return Poll::Ready(Ok(config));
                                }
                                Err(e) => return Poll::Ready(Err(e.into())),
                            }
                        }
                        Err(e) => return Poll::Ready(Err(e.into())),
                    }
                }

                LoadState::Done => {
                    panic!("RuntimeLoaderCall polled after completion");
                }
            }
        }
    }
}
