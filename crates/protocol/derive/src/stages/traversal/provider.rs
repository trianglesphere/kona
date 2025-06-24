//! L1TraversalProvider is a provider for the L1Traversal stage.
//! It is used to retrieve the next L1 block and the origin block.
//! It is also used to advance the origin block.
//! It is also used to signal the L1Traversal stage.

use crate::{
    ChainProvider, L1RetrievalProvider, L1Traversal, ManagedTraversal, OriginAdvancer,
    OriginProvider, PipelineResult, Signal, SignalReceiver,
};
use alloc::{boxed::Box, sync::Arc};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::BlockInfo;

/// `TraversalProvider` is a mux between the autonomous `L1Traversal` stage and the
/// externally-controlled `ManagedTraversal` stage.
///
/// - Before interop/managed mode, it delegates to `L1Traversal` for autonomous L1 chain traversal.
/// - After switching to managed mode, it delegates to `ManagedTraversal`, which only advances when
///   instructed externally.
///
/// This allows the pipeline to seamlessly switch between normal and managed operation.
#[derive(Debug)]
pub struct TraversalProvider<P: ChainProvider> {
    /// The rollup configuration.
    pub rollup_config: Arc<RollupConfig>,
    /// The chain provider, only Some before either stage is initialized.
    provider: Option<P>,
    /// The autonomous L1 traversal stage (used before interop/managed mode).
    l1_traversal: Option<L1Traversal<P>>,
    /// The managed L1 traversal stage (used after interop/managed mode).
    managed_traversal: Option<ManagedTraversal<P>>,
}

impl<P: ChainProvider + Clone> TraversalProvider<P> {
    /// Create a new mux provider from the given chain provider, rollup config, and origin.
    pub fn new(provider: P, rollup_config: Arc<RollupConfig>, origin: BlockInfo) -> Self {
        Self {
            rollup_config,
            provider: Some(provider),
            l1_traversal: None,
            managed_traversal: None,
        }
        .with_initial_stage(origin)
    }

    fn with_initial_stage(mut self, origin: BlockInfo) -> Self {
        // On first construction, always start in the correct mode for the given origin.
        let is_managed = self.rollup_config.is_interop_active(origin.timestamp);
        if let Some(provider) = self.provider.take() {
            if is_managed {
                let mut managed = ManagedTraversal::new(provider, Arc::clone(&self.rollup_config));
                managed.block = Some(origin);
                self.managed_traversal = Some(managed);
            } else {
                let mut l1 = L1Traversal::new(provider, Arc::clone(&self.rollup_config));
                l1.block = Some(origin);
                self.l1_traversal = Some(l1);
            }
        }
        self
    }

    /// Attempts to update the active stage of the mux, similar to ChannelProvider.
    pub fn attempt_update(&mut self)
    where
        P: Clone,
    {
        let origin = self.origin();
        let is_managed =
            origin.map(|b| self.rollup_config.is_interop_active(b.timestamp)).unwrap_or(false);
        if let Some(provider) = self.provider.take() {
            // On the first call, initialize the correct stage.
            if is_managed {
                let mut managed = ManagedTraversal::new(provider, Arc::clone(&self.rollup_config));
                managed.block = origin;
                self.managed_traversal = Some(managed);
            } else {
                let mut l1 = L1Traversal::new(provider, Arc::clone(&self.rollup_config));
                l1.block = origin;
                self.l1_traversal = Some(l1);
            }
        } else if self.l1_traversal.is_some() && is_managed {
            // Transition to managed mode.
            let l1 = self.l1_traversal.take().expect("Must have l1_traversal");
            let mut managed =
                ManagedTraversal::new(l1.data_source, Arc::clone(&self.rollup_config));
            managed.block = l1.block;
            self.managed_traversal = Some(managed);
            self.l1_traversal = None;
        } else if self.managed_traversal.is_some() && !is_managed {
            // Transition back to autonomous mode.
            let managed = self.managed_traversal.take().expect("Must have managed_traversal");
            let mut l1 = L1Traversal::new(managed.data_source, Arc::clone(&self.rollup_config));
            l1.block = managed.block;
            self.l1_traversal = Some(l1);
            self.managed_traversal = None;
        }
    }

    const fn active_l1(&mut self) -> Option<&mut L1Traversal<P>> {
        self.l1_traversal.as_mut()
    }

    const fn active_managed(&mut self) -> Option<&mut ManagedTraversal<P>> {
        self.managed_traversal.as_mut()
    }
}

#[async_trait]
impl<P: ChainProvider + Send + Sync + Clone> L1RetrievalProvider for TraversalProvider<P> {
    async fn next_l1_block(&mut self) -> PipelineResult<Option<BlockInfo>> {
        self.attempt_update();
        if let Some(m) = self.active_managed() {
            m.next_l1_block().await
        } else if let Some(l1) = self.active_l1() {
            l1.next_l1_block().await
        } else {
            Err(crate::PipelineError::MissingOrigin.crit())
        }
    }

    fn batcher_addr(&self) -> alloy_primitives::Address {
        self.managed_traversal.as_ref().map_or_else(
            || {
                self.l1_traversal
                    .as_ref()
                    .map_or(alloy_primitives::Address::default(), |l1| l1.batcher_addr())
            },
            |m| m.batcher_addr(),
        )
    }
}

impl<P: ChainProvider> OriginProvider for TraversalProvider<P> {
    fn origin(&self) -> Option<BlockInfo> {
        self.managed_traversal
            .as_ref()
            .map_or_else(|| self.l1_traversal.as_ref().and_then(|l1| l1.origin()), |m| m.origin())
    }
}

#[async_trait]
impl<P: ChainProvider + Send + Sync + Clone> OriginAdvancer for TraversalProvider<P> {
    async fn advance_origin(&mut self) -> PipelineResult<()> {
        self.attempt_update();
        if let Some(m) = self.active_managed() {
            m.advance_origin().await
        } else if let Some(l1) = self.active_l1() {
            l1.advance_origin().await
        } else {
            Err(crate::PipelineError::MissingOrigin.crit())
        }
    }
}

#[async_trait]
impl<P: ChainProvider + Send + Sync + Clone> SignalReceiver for TraversalProvider<P> {
    async fn signal(&mut self, signal: Signal) -> PipelineResult<()> {
        self.attempt_update();
        if let Some(m) = self.active_managed() {
            m.signal(signal).await
        } else if let Some(l1) = self.active_l1() {
            l1.signal(signal).await
        } else {
            Err(crate::PipelineError::MissingOrigin.crit())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::TestChainProvider;

    fn make_block(number: u64, timestamp: u64) -> BlockInfo {
        BlockInfo { number, timestamp, ..BlockInfo::default() }
    }

    fn make_provider_with_origin(
        origin: BlockInfo,
        interop_time: Option<u64>,
    ) -> TraversalProvider<TestChainProvider> {
        let mut l1_provider = TestChainProvider::default();
        l1_provider.insert_block(origin.number, origin);
        let mut cfg = RollupConfig::default();
        cfg.hardforks.interop_time = interop_time;
        TraversalProvider::new(l1_provider, Arc::new(cfg), origin)
    }

    #[test]
    fn test_autonomous_mode_before_interop() {
        let origin = make_block(1, 100);
        let mut provider = make_provider_with_origin(origin, Some(200));
        provider.attempt_update();
        assert!(provider.l1_traversal.is_some(), "Should have l1_traversal");
        assert_eq!(provider.origin(), Some(origin));
    }

    #[test]
    fn test_managed_mode_after_interop() {
        let origin = make_block(1, 300);
        let mut provider = make_provider_with_origin(origin, Some(200));
        provider.attempt_update();
        assert!(provider.managed_traversal.is_some(), "Should have managed_traversal");
        assert_eq!(provider.origin(), Some(origin));
    }

    #[test]
    fn test_transition_to_managed_mode() {
        let origin = make_block(1, 199);
        let mut provider = make_provider_with_origin(origin, Some(200));
        provider.attempt_update();
        assert!(provider.l1_traversal.is_some(), "Should have l1_traversal");
        // Simulate advancing to interop
        let new_origin = make_block(2, 200);
        provider.l1_traversal.as_mut().expect("Must have l1_traversal").block = Some(new_origin);
        provider.attempt_update();
        assert!(provider.managed_traversal.is_some(), "Should have managed_traversal");
    }

    #[tokio::test]
    async fn test_transition_back_to_autonomous_mode() {
        let origin = make_block(1, 300);
        let mut provider = make_provider_with_origin(origin, Some(200));
        provider.attempt_update();
        assert!(provider.managed_traversal.is_some(), "Should have managed_traversal");
        // Simulate reorg to before interop
        let new_origin = make_block(2, 100);
        // Send a reset signal to update managed_traversal's origin as well
        provider
            .signal(Signal::Reset(crate::types::ResetSignal {
                l1_origin: new_origin,
                system_config: Some(kona_genesis::SystemConfig::default()),
                ..Default::default()
            }))
            .await
            .unwrap();
        provider.attempt_update();
        assert!(provider.l1_traversal.is_some(), "Should have l1_traversal");
    }
}
