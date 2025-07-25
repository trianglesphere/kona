//! The internal state of the engine controller.

use crate::Metrics;
use alloy_rpc_types_engine::ForkchoiceState;
use kona_protocol::L2BlockInfo;
use serde::{Deserialize, Serialize};

/// Synchronization state tracking different blockchain head pointers for an OP Stack rollup.
///
/// The engine maintains multiple head pointers that represent different levels of
/// blockchain finality and cross-chain verification. This multi-head model enables
/// the rollup to operate with different safety guarantees while maintaining
/// compatibility with L1 finality.
///
/// ## Head Hierarchy (by Safety Level)
///
/// 1. **Finalized Head** (Highest Safety)
///    - Derived from finalized L1 data  
///    - Only has finalized L1 dependencies
///    - Cannot be reverted under normal conditions
///    - Updates every ~12 minutes with L1 finalization (Ethereum Mainnet; timing varies by L1 DA provider)
///
/// 2. **Safe Head** (High Safety)
///    - Derived from L1 and cross-verified
///    - Has cross-safe dependencies confirmed
///    - Resistant to L1 reorgs within safe confirmation depth
///    - Updates as L1 derivation proceeds
///
/// 3. **Local Safe Head** (Medium Safety)  
///    - Derived locally from L1 data
///    - Completed span-batch but not cross-verified
///    - May be reverted if cross-verification fails
///    - Pre-interop: same as safe head
///
/// 4. **Cross-Unsafe Head** (Low Safety)
///    - Cross-verified unsafe head
///    - Pre-interop: always equal to unsafe head
///    - Post-interop: verified against cross-chain dependencies
///    - May be behind unsafe head during verification delays
///
/// 5. **Unsafe Head** (Lowest Safety)
///    - Most recent block from P2P network
///    - Not yet verified against L1 data
///    - May be invalid or subject to reorg
///    - Updates immediately upon block receipt
///
/// ## Invariants
///
/// The engine maintains these ordering invariants:
/// ```ignore
/// finalized_head <= safe_head <= local_safe_head <= cross_unsafe_head <= unsafe_head
/// ```
///
/// These invariants ensure:
/// - **Safety Progression**: Higher safety levels never regress beyond lower ones  
/// - **Finality Ordering**: Finalized state is always the most conservative
/// - **Consistency**: State updates maintain logical blockchain progression
///
/// ## Cross-Chain Verification (Interop)
///
/// In interop mode, cross-verification ensures dependencies across multiple chains:
/// - **Cross-Unsafe**: Unsafe blocks verified against cross-chain dependencies
/// - **Cross-Safe**: Safe blocks with confirmed cross-chain safety
/// - **Cross-Finalized**: Finalized blocks with all cross-chain dependencies finalized
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct EngineSyncState {
    /// Most recent block found on the p2p network
    unsafe_head: L2BlockInfo,
    /// Cross-verified unsafe head, always equal to the unsafe head pre-interop
    cross_unsafe_head: L2BlockInfo,
    /// Derived from L1, and known to be a completed span-batch,
    /// but not cross-verified yet.
    local_safe_head: L2BlockInfo,
    /// Derived from L1 and cross-verified to have cross-safe dependencies.
    safe_head: L2BlockInfo,
    /// Derived from finalized L1 data,
    /// and cross-verified to only have finalized dependencies.
    finalized_head: L2BlockInfo,
}

impl EngineSyncState {
    /// Returns the current unsafe head.
    pub const fn unsafe_head(&self) -> L2BlockInfo {
        self.unsafe_head
    }

    /// Returns the current cross-verified unsafe head.
    pub const fn cross_unsafe_head(&self) -> L2BlockInfo {
        self.cross_unsafe_head
    }

    /// Returns the current local safe head.
    pub const fn local_safe_head(&self) -> L2BlockInfo {
        self.local_safe_head
    }

    /// Returns the current safe head.
    pub const fn safe_head(&self) -> L2BlockInfo {
        self.safe_head
    }

    /// Returns the current finalized head.
    pub const fn finalized_head(&self) -> L2BlockInfo {
        self.finalized_head
    }

    /// Creates a `ForkchoiceState`
    ///
    /// - `head_block` = `unsafe_head`
    /// - `safe_block` = `safe_head`
    /// - `finalized_block` = `finalized_head`
    ///
    /// If the block info is not yet available, the default values are used.
    pub const fn create_forkchoice_state(&self) -> ForkchoiceState {
        ForkchoiceState {
            head_block_hash: self.unsafe_head.hash(),
            safe_block_hash: self.safe_head.hash(),
            finalized_block_hash: self.finalized_head.hash(),
        }
    }

    /// Applies partial updates to the sync state while preserving unchanged values.
    ///
    /// This method enables atomic updates of specific head pointers without affecting
    /// others. Each head pointer is updated only if the corresponding field in the
    /// update struct is `Some`, otherwise the current value is preserved.
    ///
    /// ## Update Semantics
    /// - **Partial Updates**: Only specified heads are modified
    /// - **Atomic Application**: All updates applied together or none at all
    /// - **Metrics Integration**: Automatically records head advancement metrics
    /// - **Invariant Preservation**: Caller responsible for maintaining head ordering
    ///
    /// ## Parameters
    /// - `sync_state_update`: Contains optional updates for each head pointer
    ///
    /// ## Returns
    /// New sync state with updates applied and unchanged values preserved
    ///
    /// ## Example Usage
    /// ```ignore
    /// let update = EngineSyncStateUpdate {
    ///     unsafe_head: Some(new_unsafe_block),
    ///     safe_head: Some(new_safe_block),
    ///     ..Default::default()
    /// };
    /// let new_state = current_state.apply_update(update);
    /// ```
    ///
    /// ## Metrics Side Effects
    /// Updates automatically record metrics for:
    /// - Block number advancement for each updated head
    /// - Head pointer state transitions
    /// - Sync progress indicators
    pub fn apply_update(self, sync_state_update: EngineSyncStateUpdate) -> Self {
        if let Some(unsafe_head) = sync_state_update.unsafe_head {
            Self::update_block_label_metric(
                Metrics::UNSAFE_BLOCK_LABEL,
                unsafe_head.block_info.number,
            );
        }
        if let Some(cross_unsafe_head) = sync_state_update.cross_unsafe_head {
            Self::update_block_label_metric(
                Metrics::CROSS_UNSAFE_BLOCK_LABEL,
                cross_unsafe_head.block_info.number,
            );
        }
        if let Some(local_safe_head) = sync_state_update.local_safe_head {
            Self::update_block_label_metric(
                Metrics::LOCAL_SAFE_BLOCK_LABEL,
                local_safe_head.block_info.number,
            );
        }
        if let Some(safe_head) = sync_state_update.safe_head {
            Self::update_block_label_metric(Metrics::SAFE_BLOCK_LABEL, safe_head.block_info.number);
        }
        if let Some(finalized_head) = sync_state_update.finalized_head {
            Self::update_block_label_metric(
                Metrics::FINALIZED_BLOCK_LABEL,
                finalized_head.block_info.number,
            );
        }

        Self {
            unsafe_head: sync_state_update.unsafe_head.unwrap_or(self.unsafe_head),
            cross_unsafe_head: sync_state_update
                .cross_unsafe_head
                .unwrap_or(self.cross_unsafe_head),
            local_safe_head: sync_state_update.local_safe_head.unwrap_or(self.local_safe_head),
            safe_head: sync_state_update.safe_head.unwrap_or(self.safe_head),
            finalized_head: sync_state_update.finalized_head.unwrap_or(self.finalized_head),
        }
    }

    /// Updates a block label metric, keyed by the label.
    #[inline]
    fn update_block_label_metric(label: &'static str, number: u64) {
        kona_macros::set!(gauge, Metrics::BLOCK_LABELS, "label", label, number as f64);
    }
}

/// Partial update specification for engine synchronization state.
///
/// This structure enables selective updates of specific head pointers in the engine
/// sync state without requiring all heads to be modified simultaneously. Each field
/// is optional, allowing fine-grained control over which heads are updated.
///
/// ## Design Principles
/// - **Selective Updates**: Only modify specified head pointers
/// - **Backward Compatibility**: Unspecified heads retain current values
/// - **Atomic Semantics**: All specified updates applied together
/// - **Type Safety**: Optional fields prevent accidental overwrites
///
/// ## Usage Patterns
///
/// ### Single Head Update
/// ```ignore
/// let update = EngineSyncStateUpdate {
///     unsafe_head: Some(new_block),
///     ..Default::default()
/// };
/// ```
///
/// ### Multiple Head Update
/// ```ignore  
/// let update = EngineSyncStateUpdate {
///     safe_head: Some(new_safe),
///     finalized_head: Some(new_finalized),
///     ..Default::default()
/// };
/// ```
///
/// ### Full State Sync
/// ```ignore
/// let update = EngineSyncStateUpdate {
///     unsafe_head: Some(unsafe_block),
///     cross_unsafe_head: Some(cross_unsafe_block),
///     local_safe_head: Some(local_safe_block),
///     safe_head: Some(safe_block),
///     finalized_head: Some(finalized_block),
/// };
/// ```
///
/// ## Error Prevention
/// Using optional fields helps prevent common errors:
/// - **Accidental Overwrites**: Must explicitly specify each update
/// - **Default Value Issues**: No risk of using uninitialized values
/// - **Partial Update Bugs**: Clear distinction between "update" and "keep current"
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineSyncStateUpdate {
    /// Most recent block found on the p2p network
    pub unsafe_head: Option<L2BlockInfo>,
    /// Cross-verified unsafe head, always equal to the unsafe head pre-interop
    pub cross_unsafe_head: Option<L2BlockInfo>,
    /// Derived from L1, and known to be a completed span-batch,
    /// but not cross-verified yet.
    pub local_safe_head: Option<L2BlockInfo>,
    /// Derived from L1 and cross-verified to have cross-safe dependencies.
    pub safe_head: Option<L2BlockInfo>,
    /// Derived from finalized L1 data,
    /// and cross-verified to only have finalized dependencies.
    pub finalized_head: Option<L2BlockInfo>,
}

/// Complete state of the engine controller, encompassing synchronization and operational status.
///
/// The engine state combines blockchain synchronization information with operational
/// flags that control engine behavior. This unified state model enables atomic
/// updates and consistent decision-making across all engine operations.
///
/// ## State Components
///
/// ### Synchronization State ([`EngineSyncState`])
/// Tracks multiple blockchain head pointers representing different safety levels:
/// - **Unsafe Head**: Latest P2P gossip (may be invalid)
/// - **Cross-Unsafe Head**: Cross-verified unsafe head (interop mode)
/// - **Local Safe Head**: L1-derived, completed span-batch
/// - **Safe Head**: L1-derived and cross-verified
/// - **Finalized Head**: Derived from finalized L1 data
///
/// ### Execution Layer Status
/// Tracks whether the execution layer has completed initial synchronization:
/// - **Syncing**: EL is still catching up to current head
/// - **Synced**: EL is fully synchronized and ready for normal operation
///
/// ### Reorg Recovery Flags
/// Controls special forkchoice update behavior during chain reorganizations:
/// - **Backup Unsafe Reorg**: Forces forkchoice update during unsafe chain recovery
/// - **Context**: Invalid span batches, sequencer errors, L1 reorgs
///
/// ## State Transitions
///
/// State updates occur through several mechanisms:
/// 1. **Task Completion**: Successful tasks update relevant state components
/// 2. **External Events**: L1 finality, P2P gossip, sequencer blocks
/// 3. **Error Recovery**: Reset operations, reorg handling, sync failures
/// 4. **Configuration Changes**: System config updates, fork activations
///
/// ## Consistency Guarantees
///
/// The engine maintains several invariants:
/// - **Head Ordering**: Finalized ≤ Safe ≤ Local Safe ≤ Cross-Unsafe ≤ Unsafe
/// - **EL Sync Logic**: Sync status affects task execution priorities
/// - **Recovery State**: Reorg flags prevent inconsistent forkchoice updates
///
/// ## Decision Making
///
/// Engine state drives operational decisions:
/// - **Consolidation**: Whether unsafe head needs L1 validation
/// - **Task Priorities**: EL sync status affects scheduling
/// - **Error Handling**: Recovery flags influence retry logic
/// - **Forkchoice Updates**: Sync state determines forkchoice parameters
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct EngineState {
    /// The sync state of the engine.
    pub sync_state: EngineSyncState,

    /// Whether or not the EL has finished syncing.
    pub el_sync_finished: bool,

    /// Track when the rollup node changes the forkchoice to restore previous
    /// known unsafe chain. e.g. Unsafe Reorg caused by Invalid span batch.
    /// This update does not retry except engine returns non-input error
    /// because engine may forgot backupUnsafeHead or backupUnsafeHead is not part
    /// of the chain.
    pub need_fcu_call_backup_unsafe_reorg: bool,
}

impl EngineState {
    /// Determines whether consolidation is needed to advance the safe chain.
    ///
    /// [Consolidation] is the process of validating unsafe blocks against L1-derived
    /// data to potentially advance the safe head. This process is only needed when
    /// the unsafe chain has progressed beyond the safe chain.
    ///
    /// ## Consolidation Logic
    ///
    /// ### When Consolidation is Needed (`true`)
    /// - **Unsafe ahead of Safe**: `unsafe_head.number > safe_head.number`
    /// - **Purpose**: Validate unsafe blocks against L1 derivation data
    /// - **Process**: Compare derived attributes with actual unsafe block
    /// - **Outcome**: Advance safe head if attributes match, rebuild block if not
    ///
    /// ### When Consolidation is Not Needed (`false`)
    /// - **Heads Equal**: `unsafe_head == safe_head`
    /// - **Alternative**: Use [`BuildTask`] for new block creation
    /// - **Context**: Normal sequencer operation, no pending validation
    ///
    /// ## State Implications
    ///
    /// ### Consolidation Required
    /// ```ignore
    /// unsafe_head: Block #100
    /// safe_head:   Block #98
    /// // Need to validate blocks #99 and #100 against L1 data
    /// ```
    ///
    /// ### No Consolidation Required
    /// ```ignore
    /// unsafe_head: Block #100  
    /// safe_head:   Block #100
    /// // Chains are synchronized, can build new block #101
    /// ```
    ///
    /// ## Performance Considerations
    /// - **Consolidation**: More expensive, involves L1 derivation and validation
    /// - **Direct Building**: Faster, skips validation for already-safe blocks
    /// - **Batching**: Multiple blocks may be consolidated in sequence
    ///
    /// [Consolidation]: https://specs.optimism.io/protocol/derivation.html#l1-consolidation-payload-attributes-matching
    /// [`BuildTask`]: crate::BuildTask
    pub fn needs_consolidation(&self) -> bool {
        self.sync_state.safe_head() != self.sync_state.unsafe_head()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Metrics;
    use kona_protocol::BlockInfo;
    use metrics_exporter_prometheus::PrometheusBuilder;
    use rstest::rstest;

    impl EngineState {
        /// Set the unsafe head.
        pub fn set_unsafe_head(&mut self, unsafe_head: L2BlockInfo) {
            self.sync_state.apply_update(EngineSyncStateUpdate {
                unsafe_head: Some(unsafe_head),
                ..Default::default()
            });
        }

        /// Set the cross-verified unsafe head.
        pub fn set_cross_unsafe_head(&mut self, cross_unsafe_head: L2BlockInfo) {
            self.sync_state.apply_update(EngineSyncStateUpdate {
                cross_unsafe_head: Some(cross_unsafe_head),
                ..Default::default()
            });
        }

        /// Set the local safe head.
        pub fn set_local_safe_head(&mut self, local_safe_head: L2BlockInfo) {
            self.sync_state.apply_update(EngineSyncStateUpdate {
                local_safe_head: Some(local_safe_head),
                ..Default::default()
            });
        }

        /// Set the safe head.
        pub fn set_safe_head(&mut self, safe_head: L2BlockInfo) {
            self.sync_state.apply_update(EngineSyncStateUpdate {
                safe_head: Some(safe_head),
                ..Default::default()
            });
        }

        /// Set the finalized head.
        pub fn set_finalized_head(&mut self, finalized_head: L2BlockInfo) {
            self.sync_state.apply_update(EngineSyncStateUpdate {
                finalized_head: Some(finalized_head),
                ..Default::default()
            });
        }
    }

    #[rstest]
    #[case::set_unsafe(EngineState::set_unsafe_head, Metrics::UNSAFE_BLOCK_LABEL, 1)]
    #[case::set_cross_unsafe(
        EngineState::set_cross_unsafe_head,
        Metrics::CROSS_UNSAFE_BLOCK_LABEL,
        2
    )]
    #[case::set_local_safe(EngineState::set_local_safe_head, Metrics::LOCAL_SAFE_BLOCK_LABEL, 3)]
    #[case::set_safe_head(EngineState::set_safe_head, Metrics::SAFE_BLOCK_LABEL, 4)]
    #[case::set_finalized_head(EngineState::set_finalized_head, Metrics::FINALIZED_BLOCK_LABEL, 5)]
    #[cfg(feature = "metrics")]
    fn test_chain_label_metrics(
        #[case] set_fn: impl Fn(&mut EngineState, L2BlockInfo),
        #[case] label_name: &str,
        #[case] number: u64,
    ) {
        let handle = PrometheusBuilder::new().install_recorder().unwrap();
        crate::Metrics::init();

        let mut state = EngineState::default();
        set_fn(
            &mut state,
            L2BlockInfo {
                block_info: BlockInfo { number, ..Default::default() },
                ..Default::default()
            },
        );

        assert!(handle.render().contains(
            format!("kona_node_block_labels{{label=\"{label_name}\"}} {number}").as_str()
        ));
    }
}
