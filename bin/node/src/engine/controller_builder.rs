//! An [EngineController] builder.

use anyhow::{Result, bail};
use kona_genesis::RollupConfig;

use crate::{
    engine::{EngineClient, EngineController, EngineState, SyncStatus},
    sync::SyncConfig,
};

/// A builder for the [EngineController].
#[derive(Debug, Clone)]
pub struct ControllerBuilder {
    /// The engine client.
    client: EngineClient,
    /// The engine state.
    state: Option<EngineState>,
    /// The rollup config.
    config: Option<RollupConfig>,
    /// The sync config.
    sync: Option<SyncConfig>,
}

impl ControllerBuilder {
    /// Instantiates a new [ControllerBuilder] from the provided [EngineClient].
    pub fn new(client: EngineClient) -> Self {
        Self { client, state: None, config: None, sync: None }
    }

    /// Sets the [RollupConfig] for the [EngineController].
    pub fn with_config(mut self, config: RollupConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Sets the [SyncConfig] for the [EngineController].
    pub fn with_sync(mut self, sync: SyncConfig) -> Self {
        self.sync = Some(sync);
        self
    }

    /// Builds the [EngineController].
    pub async fn build(self) -> Result<EngineController> {
        let state = if let Some(s) = self.state {
            s
        } else {
            EngineState::builder(self.client.clone()).build().await?
        };

        let config = if let Some(c) = self.config {
            c
        } else {
            bail!("RollupConfig is required to build the EngineController");
        };

        let sync = if let Some(s) = self.sync {
            s
        } else {
            bail!("SyncConfig is required to build the EngineController");
        };

        Ok(EngineController {
            client: self.client,
            sync_status: SyncStatus::from(sync.sync_mode),
            state,
            blocktime: config.block_time,
            ecotone_timestamp: config.hardforks.ecotone_time,
            canyon_timestamp: config.hardforks.canyon_time,
        })
    }
}
