//! The [`Engine`] is a task queue that receives and executes [`EngineTask`]s.

use super::{EngineTaskError, EngineTaskExt, ForkchoiceTask};
use crate::{EngineClient, EngineState, EngineTask, EngineTaskType};
use alloy_provider::{Provider, RootProvider};
use kona_genesis::{RollupConfig, SystemConfig};
use kona_protocol::{BlockInfo, L2BlockInfo, to_system_config};
use kona_sources::{SyncStartError, find_starting_forkchoice};
use op_alloy_consensus::OpTxEnvelope;
use op_alloy_network::Optimism;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use thiserror::Error;
use tokio::sync::watch::Sender;

/// The [`Engine`] task queue.
///
/// Tasks are processed in FIFO order, providing synchronization and ordering guarantees
/// for the L2 execution layer and other actors. Because tasks are executed one at a time,
/// they are considered to be atomic operations over the [`EngineState`], and are given
/// exclusive access to the engine state during execution.
///
/// Tasks within the queue are also considered fallible. If they fail with a temporary error,
/// they are not popped from the queue and are retried on the next call to [`Engine::drain`].
#[derive(Debug)]
pub struct Engine {
    /// The state of the engine.
    state: EngineState,
    /// A sender that can be used to notify the engine actor of state changes.
    state_sender: Sender<EngineState>,
    /// The task queue.
    tasks: HashMap<EngineTaskType, VecDeque<EngineTask>>,
    /// The current task being executed.
    cursor: EngineTaskType,
    /// The rollup configuration, used for handling reset events.
    config: Arc<RollupConfig>,
    /// The engine client, used for handling reset events.
    engine_client: Arc<EngineClient>,
    /// The L1 provider, used for handling reset events.
    l1_provider: RootProvider,
    /// The L2 provider, used for handling reset events.
    l2_provider: RootProvider<Optimism>,
}

impl Engine {
    /// Creates a new [`Engine`] with an empty task queue and the passed initial [`EngineState`].
    ///
    /// An initial [`EngineTask::ForkchoiceUpdate`] is added to the task queue to synchronize the
    /// engine with the forkchoice state of the [`EngineState`].
    pub fn new(
        initial_state: EngineState,
        state_sender: Sender<EngineState>,
        config: Arc<RollupConfig>,
        engine_client: EngineClient,
        l1_provider: RootProvider,
        l2_provider: RootProvider<Optimism>,
    ) -> Self {
        Self {
            state: initial_state,
            tasks: HashMap::new(),
            cursor: EngineTaskType::ForkchoiceUpdate,
            state_sender,
            config,
            engine_client: Arc::new(engine_client),
            l1_provider,
            l2_provider,
        }
    }

    /// Enqueues a new [`EngineTask`] for execution.
    pub fn enqueue(&mut self, task: EngineTask) {
        self.tasks.entry(task.ty()).or_default().push_back(task);
    }

    /// Returns a reference to the inner [`EngineState`].
    pub const fn state(&self) -> &EngineState {
        &self.state
    }

    /// Returns the L2 Safe Head [`L2BlockInfo`] from the state.
    pub const fn safe_head(&self) -> L2BlockInfo {
        self.state.safe_head()
    }

    /// Clears the task queue.
    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    /// Returns the next task type to be executed.
    pub fn next(&self) -> EngineTaskType {
        let mut ty = self.cursor;
        let task_len = self.tasks.len();
        for _ in 0..task_len {
            if !self.tasks.contains_key(&ty) {
                ty = ty.next();
            } else {
                break;
            }
        }
        ty
    }

    /// Returns a receiver that can be used to listen to engine state updates.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<EngineState> {
        self.state_sender.subscribe()
    }

    /// Issues a reset to the engine.
    pub async fn reset(&mut self) -> Result<(L2BlockInfo, BlockInfo, SystemConfig), EngineError> {
        // Clear the task queue.
        self.clear();

        // Find a plausible sync starting point to reset to.
        let start =
            find_starting_forkchoice(&self.config, &self.l1_provider, &self.l2_provider).await?;

        // Update the engine state to reflect the updated chain.
        self.state.set_unsafe_head(start.un_safe);
        self.state.set_safe_head(start.safe);
        self.state.set_finalized_head(start.finalized);

        // Enqueue a forkchoice update to equivocate the new chain with the EL.
        self.enqueue(EngineTask::ForkchoiceUpdate(
            ForkchoiceTask::new(self.engine_client.clone()).exclude_safe_and_finalized(),
        ));

        // Fetch the L1 origin block.
        let l1_origin: BlockInfo = self
            .l1_provider
            .get_block(start.safe.l1_origin.hash.into())
            .await
            .unwrap()
            .map(|b| b.into_consensus())
            .map(BlockInfo::from)
            .ok_or(SyncStartError::BlockNotFound(start.safe.l1_origin.number.into()))?;

        // Fetch the full L2 safe block and construct the SystemConfig at that block
        let l2_safe_block = self
            .l2_provider
            .get_block(start.safe.block_info.hash.into())
            .full()
            .await
            .unwrap()
            .ok_or(SyncStartError::BlockNotFound(start.safe.block_info.number.into()))?
            .into_consensus()
            .map_transactions(|t| {
                <alloy_rpc_types_eth::Transaction<OpTxEnvelope> as Clone>::clone(&t).into_inner()
            });
        let system_config = to_system_config(&l2_safe_block, &self.config).unwrap();

        Ok((start.safe, l1_origin, system_config))
    }

    /// Attempts to drain the queue by executing all [`EngineTask`]s in-order. If any task returns
    /// an error along the way, it is not popped from the queue (in case it must be retried) and
    /// the error is returned.
    ///
    /// If an [`EngineTaskError::Reset`] is encountered, the remaining tasks in the queue are
    /// cleared.
    pub async fn drain(&mut self) -> Result<(), EngineError> {
        loop {
            let ty = self.next();
            self.cursor = self.cursor.next();
            let Some(task) = self.tasks.get(&ty) else {
                return Ok(());
            };
            let Some(task) = task.front() else {
                return Ok(());
            };
            match task.execute(&mut self.state).await {
                Ok(_) => {}
                Err(EngineTaskError::Reset(_)) => {
                    let (safe_head, l1_origin, system_config) = self.reset().await?;
                    return Err(EngineError::Reset {
                        new_safe_head: safe_head,
                        safe_head_l1_origin: l1_origin,
                        system_config,
                    });
                }
                e => e?,
            }

            // Update the state and notify the engine actor.
            self.state_sender.send_replace(self.state);

            let ty = task.ty();
            if let Some(queue) = self.tasks.get_mut(&ty) {
                queue.pop_front();
            };
        }
    }
}

/// An error that can occur within the [`Engine`].
#[derive(Debug, Error)]
#[allow(clippy::large_enum_variant)]
pub enum EngineError {
    /// An error occurred within an engine task.
    #[error(transparent)]
    Task(#[from] EngineTaskError),
    /// An error occurred during finding a plausible sync starting point during an engine reset.
    #[error(transparent)]
    SyncStart(#[from] SyncStartError),
    /// A reset event was triggered, and the engine has reorganized its state.
    #[error("Engine triggered reset event")]
    Reset {
        /// The new safe head to reset to.
        new_safe_head: L2BlockInfo,
        /// The new safe head's L1 origin block.
        safe_head_l1_origin: BlockInfo,
        /// The new safe head's SystemConfig
        system_config: SystemConfig,
    },
}
