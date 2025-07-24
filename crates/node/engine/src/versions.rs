//! Engine API version selection logic for OP Stack rollup operations.
//!
//! The Engine API has evolved through multiple versions, each introducing new features
//! and payload formats. This module provides automatic version selection based on
//! rollup configuration and block timestamps, ensuring compatibility with different
//! hardfork activations.
//!
//! ## Version Evolution
//!
//! ### Engine API V2 (Bedrock → Canyon, Delta)
//! - **Payload Format**: Basic execution payloads without beacon chain integration
//! - **Features**: Core block building, forkchoice updates, payload retrieval
//! - **Limitations**: No parent beacon block root support
//!
//! ### Engine API V3 (Ecotone → Fjord, Granite)  
//! - **Payload Format**: Adds parent beacon block root for EIP-4788 support
//! - **Features**: Beacon chain state integration, improved validation
//! - **Activation**: Ecotone hardfork (Cancun L1 equivalent)
//!
//! ### Engine API V4 (Isthmus+)
//! - **Payload Format**: Enhanced payload structure for future features
//! - **Features**: Additional validation, extended metadata support
//! - **Activation**: Isthmus hardfork (future protocol upgrade)
//!
//! ## Version Selection Strategy
//!
//! Version selection is **timestamp-based** using the rollup configuration:
//! ```ignore
//! if cfg.is_isthmus_active(timestamp) {
//!     use_engine_api_v4()
//! } else if cfg.is_ecotone_active(timestamp) {
//!     use_engine_api_v3()  
//! } else {
//!     use_engine_api_v2()
//! }
//! ```
//!
//! ## Compatibility Considerations
//!
//! ### Execution Layer Requirements
//! - **V2**: Supported by all OP Stack execution layers
//! - **V3**: Requires Ecotone-compatible EL (parent beacon root support)
//! - **V4**: Requires Isthmus-compatible EL (future upgrade)
//!
//! ### Protocol Coordination
//! - **L1 Synchronization**: Version must match L1 hardfork activation schedule
//! - **Cross-Chain Consistency**: All nodes must use same version at given timestamp
//! - **Upgrade Coordination**: Version switches automatically at hardfork boundaries
//!
//! Adapted from the [op-node version providers][vp].
//!
//! [vp]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/types.go#L546

use kona_genesis::RollupConfig;

/// Engine API version for `engine_forkchoiceUpdated` method calls.
///
/// The forkchoice update method has evolved to support new payload formats and
/// features introduced in different hardforks. Version selection ensures that
/// the rollup node uses the appropriate API variant for the current chain state.
///
/// ## Version Differences
///
/// ### V2 (Bedrock → Delta)
/// - **Payload Support**: Basic execution payloads
/// - **Attributes**: Standard payload attributes without beacon integration
/// - **Compatibility**: Universal support across all OP Stack ELs
///
/// ### V3 (Ecotone+)
/// - **Payload Support**: Enhanced payloads with parent beacon block root
/// - **Attributes**: Extended attributes for EIP-4788 beacon state access
/// - **Compatibility**: Requires Ecotone-compatible execution layer
///
/// ## Usage Context
/// - **Forkchoice Synchronization**: Called when updating EL's canonical chain view
/// - **Block Building Initiation**: Called with attributes to start payload construction
/// - **State Transitions**: Called during head advancement and finalization updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineForkchoiceVersion {
    /// The `engine_forkchoiceUpdated` api version 2.
    V2,
    /// The `engine_forkchoiceUpdated` api version 3.
    V3,
}

impl EngineForkchoiceVersion {
    /// Returns the appropriate [`EngineForkchoiceVersion`] for the chain at the given attributes.
    ///
    /// Uses the [`RollupConfig`] to check which hardfork is active at the given timestamp.
    pub fn from_cfg(cfg: &RollupConfig, timestamp: u64) -> Self {
        if cfg.is_ecotone_active(timestamp) {
            // Cancun+
            Self::V3
        } else {
            // Bedrock, Canyon, Delta
            Self::V2
        }
    }
}

/// The method version for the `engine_newPayload` api.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineNewPayloadVersion {
    /// The `engine_newPayload` api version 2.
    V2,
    /// The `engine_newPayload` api version 3.
    V3,
    /// The `engine_newPayload` api version 4.
    V4,
}

impl EngineNewPayloadVersion {
    /// Returns the appropriate [`EngineNewPayloadVersion`] for the chain at the given timestamp.
    ///
    /// Uses the [`RollupConfig`] to check which hardfork is active at the given timestamp.
    pub fn from_cfg(cfg: &RollupConfig, timestamp: u64) -> Self {
        if cfg.is_isthmus_active(timestamp) {
            Self::V4
        } else if cfg.is_ecotone_active(timestamp) {
            // Cancun
            Self::V3
        } else {
            Self::V2
        }
    }
}

/// The method version for the `engine_getPayload` api.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineGetPayloadVersion {
    /// The `engine_getPayload` api version 2.
    V2,
    /// The `engine_getPayload` api version 3.
    V3,
    /// The `engine_getPayload` api version 4.
    V4,
}

impl EngineGetPayloadVersion {
    /// Returns the appropriate [`EngineGetPayloadVersion`] for the chain at the given timestamp.
    ///
    /// Uses the [`RollupConfig`] to check which hardfork is active at the given timestamp.
    pub fn from_cfg(cfg: &RollupConfig, timestamp: u64) -> Self {
        if cfg.is_isthmus_active(timestamp) {
            Self::V4
        } else if cfg.is_ecotone_active(timestamp) {
            // Cancun
            Self::V3
        } else {
            Self::V2
        }
    }
}
