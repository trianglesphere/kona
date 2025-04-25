//! Calling module

use alloy_primitives::{Address, B256, U256, b256};
use alloy_provider::Provider;
use alloy_rpc_types::Block;
use alloy_transport::{RpcError, TransportErrorKind};
use futures::{FutureExt, future::BoxFuture, ready};
use kona_derive::traits::ChainProvider;
use kona_genesis::RollupConfig;
use kona_protocol::BlockInfo;
use kona_providers_alloy::{AlloyChainProvider, AlloyChainProviderError};
use kona_rpc::ProtocolVersion;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

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
#[derive(Debug)]
enum LoadState {
    /// Initial state
    Init,
    /// Getting latest block number
    GettingLatestBlock(BoxFuture<'static, Result<u64, AlloyChainProviderError>>),
    /// Getting block info
    GettingBlockInfo(BoxFuture<'static, Result<BlockInfo, AlloyChainProviderError>>),
    /// Getting block hash verification
    VerifyingBlockHash {
        block_info: BlockInfo,
        future: BoxFuture<'static, Result<Option<Block>, AlloyChainProviderError>>,
    },
    /// Getting unsafe block signer
    GettingUnsafeBlockSigner {
        block_info: BlockInfo,
        future: BoxFuture<'static, Result<B256, AlloyChainProviderError>>,
    },
    /// Getting required protocol version
    GettingRequiredProtocolVersion {
        block_info: BlockInfo,
        unsafe_block_signer: Address,
        future: BoxFuture<'static, Result<B256, AlloyChainProviderError>>,
    },
    /// Getting recommended protocol version
    GettingRecommendedProtocolVersion {
        block_info: BlockInfo,
        unsafe_block_signer: Address,
        required_version: ProtocolVersion,
        future: BoxFuture<'static, Result<B256, AlloyChainProviderError>>,
    },
    /// Done loading
    Done,
}

impl core::fmt::Debug for LoadState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadState::Init => write!(f, "Init"),
            LoadState::GettingLatestBlock(_) => write!(f, "GettingLatestBlock"),
            LoadState::GettingBlockInfo(_) => write!(f, "GettingBlockInfo"),
            LoadState::VerifyingBlockHash(_, _) => write!(f, "VerifyingBlockHash"),
            LoadState::GettingUnsafeBlockSigner(_, _) => write!(f, "GettingUnsafeBlockSigner"),
            LoadState::GettingRequiredProtocolVersion(_, _, _) => {
                write!(f, "GettingRequiredProtocolVersion")
            }
            LoadState::GettingRecommendedProtocolVersion(_, _, _, _) => {
                write!(f, "GettingRecommendedProtocolVersion")
            }
            LoadState::Done => write!(f, "Done"),
        }
    }
}

impl RuntimeCall {
    /// Creates a new RuntimeCall
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

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        loop {
            match std::mem::replace(&mut this.state, LoadState::Init) {
                LoadState::Init => {
                    // If we have block info, skip to verification
                    if let Some(block_info) = this.block_info.take() {
                        let fut = this.provider.inner.get_block(block_info.hash.into());
                        this.state =
                            LoadState::VerifyingBlockHash { block_info, future: Box::pin(fut) };
                    } else {
                        let fut = this.provider.latest_block_number();
                        this.state = LoadState::GettingLatestBlock(Box::pin(fut));
                    }
                }

                LoadState::GettingLatestBlock(mut fut) => {
                    match Future::poll(Pin::new(&mut fut), cx) {
                        Poll::Ready(Ok(block_num)) => {
                            let fut = this.provider.block_info_by_number(block_num);
                            this.state = LoadState::GettingBlockInfo(Box::pin(fut));
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Pending => {
                            this.state = LoadState::GettingLatestBlock(fut);
                            return Poll::Pending;
                        }
                    }
                }

                LoadState::GettingBlockInfo(mut fut) => {
                    match Future::poll(Pin::new(&mut fut), cx) {
                        Poll::Ready(Ok(block_info)) => {
                            let fut = this.provider.inner.get_block(block_info.hash.into());
                            this.state =
                                LoadState::VerifyingBlockHash { block_info, future: Box::pin(fut) };
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Pending => {
                            this.state = LoadState::GettingBlockInfo(fut);
                            return Poll::Pending;
                        }
                    }
                }

                LoadState::VerifyingBlockHash { block_info, mut future } => {
                    match Future::poll(Pin::new(&mut future), cx) {
                        Poll::Ready(Ok(Some(block))) => {
                            if block.header.hash != block_info.hash {
                                return Poll::Ready(Err(RuntimeCallError::BlockHashMismatch {
                                    expected: block_info.hash,
                                    got: block.header.hash,
                                }));
                            }
                            let fut = this
                                .provider
                                .inner
                                .get_storage_at(
                                    this.config.l1_system_config_address,
                                    UNSAFE_BLOCK_SIGNER_ADDRESS_STORAGE_SLOT.into(),
                                )
                                .hash(block_info.hash);
                            this.state = LoadState::GettingUnsafeBlockSigner {
                                block_info,
                                future: Box::pin(fut),
                            };
                        }
                        Poll::Ready(Ok(None)) => {
                            return Poll::Ready(Err(RuntimeCallError::BlockNotFound))
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Pending => {
                            this.state = LoadState::VerifyingBlockHash { block_info, future };
                            return Poll::Pending;
                        }
                    }
                }

                LoadState::GettingUnsafeBlockSigner { block_info, mut future } => {
                    match Future::poll(Pin::new(&mut future), cx) {
                        Poll::Ready(Ok(storage)) => {
                            let unsafe_block_signer =
                                Address::from_slice(&storage.to_be_bytes_vec()[12..]);
                            if this.config.protocol_versions_address == Address::ZERO {
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
                            let fut = this
                                .provider
                                .inner
                                .get_storage_at(
                                    this.config.protocol_versions_address,
                                    REQUIRED_PROTOCOL_VERSION_STORAGE_SLOT.into(),
                                )
                                .hash(block_info.hash);
                            this.state = LoadState::GettingRequiredProtocolVersion {
                                block_info,
                                unsafe_block_signer,
                                future: Box::pin(fut),
                            };
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Pending => {
                            this.state = LoadState::GettingUnsafeBlockSigner { block_info, future };
                            return Poll::Pending;
                        }
                    }
                }

                LoadState::GettingRequiredProtocolVersion {
                    block_info,
                    unsafe_block_signer,
                    mut future,
                } => match Future::poll(Pin::new(&mut future), cx) {
                    Poll::Ready(Ok(storage)) => match ProtocolVersion::decode(storage.into()) {
                        Ok(required_version) => {
                            let fut = this
                                .provider
                                .inner
                                .get_storage_at(
                                    this.config.protocol_versions_address,
                                    RECOMMENDED_PROTOCOL_VERSION_STORAGE_SLOT.into(),
                                )
                                .hash(block_info.hash);
                            this.state = LoadState::GettingRecommendedProtocolVersion {
                                block_info,
                                unsafe_block_signer,
                                required_version,
                                future: Box::pin(fut),
                            };
                        }
                        Err(e) => return Poll::Ready(Err(e.into())),
                    },
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                    Poll::Pending => {
                        this.state = LoadState::GettingRequiredProtocolVersion {
                            block_info,
                            unsafe_block_signer,
                            future,
                        };
                        return Poll::Pending;
                    }
                },

                LoadState::GettingRecommendedProtocolVersion {
                    unsafe_block_signer,
                    required_version,
                    mut future,
                    ..
                } => match Future::poll(Pin::new(&mut future), cx) {
                    Poll::Ready(Ok(storage)) => match ProtocolVersion::decode(storage.into()) {
                        Ok(recommended_version) => {
                            let config = RuntimeConfig {
                                unsafe_block_signer_address: unsafe_block_signer,
                                required_protocol_version: required_version,
                                recommended_protocol_version: recommended_version,
                            };
                            this.state = LoadState::Done;
                            return Poll::Ready(Ok(config));
                        }
                        Err(e) => return Poll::Ready(Err(e.into())),
                    },
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                    Poll::Pending => {
                        this.state = LoadState::GettingRecommendedProtocolVersion {
                            block_info,
                            unsafe_block_signer,
                            required_version,
                            future,
                        };
                        return Poll::Pending;
                    }
                },

                LoadState::Done => {
                    panic!("RuntimeCall polled after completion");
                }
            }
        }
    }
}
