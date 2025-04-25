use crate::{RuntimeConfig, RuntimeLoader, RuntimeLoaderError};
use kona_protocol::BlockInfo;
use pin_project::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// A wrapper around a runtime loader call that implements Future and allows setting block info.
#[derive(Debug)]
#[pin_project]
pub struct RuntimeCall {
    /// The runtime loader that handles executing the runtime call.
    #[pin]
    loader: RuntimeLoader,
    /// The optional block info to use for loading the runtime.
    /// This can be set to specify a particular block for loading.
    /// If not set, the latest block will be used.
    block_info: Option<BlockInfo>,
    /// The internal state of the runtime call.
    #[pin]
    state: RuntimeCallState,
}

#[pin_project(project = RuntimeCallStateProj)]
enum RuntimeCallState {
    /// The default initial state of the runtime call.
    /// This state is used to indicate that the runtime call has not yet started.
    Initial,
    /// When the block info hasn't been set on the [`RuntimeCall`], this state is used to
    /// indicate that the runtime call is loading the latest block info prior to loading the
    /// runtime.
    LoadingLatest(#[pin] Pin<Box<dyn Future<Output = Result<RuntimeConfig, RuntimeLoaderError>>>>),
    /// When the block info has been set on the [`RuntimeCall`], this state is used to
    /// indicate that the runtime call is loading the runtime with the specified block info.
    LoadingWithBlockInfo(
        #[pin] Pin<Box<dyn Future<Output = Result<RuntimeConfig, RuntimeLoaderError>>>>,
    ),
    /// This marks the completion of the runtime call.
    /// It is used in the `RuntimeCallStateProj`, so we need to mark it as `#[allow(dead_code)]`.
    #[allow(dead_code)]
    Complete(Result<RuntimeConfig, RuntimeLoaderError>),
}

impl core::fmt::Debug for RuntimeCallState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Initial => f.debug_tuple("Initial").finish(),
            Self::LoadingLatest(_) => f.debug_tuple("LoadingLatest").finish(),
            Self::LoadingWithBlockInfo(_) => f.debug_tuple("LoadingWithBlockInfo").finish(),
            Self::Complete(_) => f.debug_tuple("Complete").finish(),
        }
    }
}

impl RuntimeCall {
    /// Creates a new RuntimeCall with the given loader.
    pub const fn new(loader: RuntimeLoader) -> Self {
        Self { loader, block_info: None, state: RuntimeCallState::Initial }
    }

    /// Sets the block info to use for loading.
    pub const fn block_info(mut self, block_info: BlockInfo) -> Self {
        self.block_info = Some(block_info);
        self
    }
}

impl Future for RuntimeCall {
    type Output = Result<RuntimeConfig, RuntimeLoaderError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let state = this.state.as_mut().project();

        match state {
            RuntimeCallStateProj::Initial => {
                if let Some(block_info) = this.block_info.take() {
                    let mut loader = this.loader.clone();
                    let fut = Box::pin(async move { loader.load_internal(block_info).await });
                    this.state.set(RuntimeCallState::LoadingWithBlockInfo(fut));
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else {
                    let mut loader = this.loader.clone();
                    let fut = Box::pin(async move { loader.load_latest().await });
                    this.state.set(RuntimeCallState::LoadingLatest(fut));
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            RuntimeCallStateProj::LoadingLatest(fut) => fut.poll(cx),
            RuntimeCallStateProj::LoadingWithBlockInfo(fut) => fut.poll(cx),
            RuntimeCallStateProj::Complete(result) => Poll::Ready(result.clone()),
        }
    }
}
