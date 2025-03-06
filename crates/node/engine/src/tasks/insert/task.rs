//! A task to insert an unsafe payload into the execution engine.

use alloy_eips::BlockNumberOrTag;
use alloy_primitives::B256;
use alloy_provider::ext::EngineApi;
use alloy_rpc_types_engine::{ExecutionPayload, ExecutionPayloadInputV2, ForkchoiceState};
use kona_protocol::L2BlockInfo;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::sync::Arc;

use tokio::{
    sync::broadcast::{Receiver as TokioReceiver, Sender as TokioSender},
    task::JoinHandle,
};

use crate::{
    EngineClient, EngineForkchoiceVersion, EngineState, EngineTask, InsertTaskError,
    InsertTaskInput, InsertTaskOut, SyncConfig, SyncStatus,
};

/// A strongly typed receiver channel.
type Receiver = TokioReceiver<InsertTaskInput>;

/// A strongly typed sender channel.
type Sender = TokioSender<InsertTaskOut>;

/// The input type for the [InsertTask].
/// The third argument is the genesis l2 hash expected to be provided by the rollup config.
type Input = (
    Sender,
    Receiver,
    Arc<EngineClient>,
    SyncConfig,
    B256,
    EngineForkchoiceVersion,
    OpNetworkPayloadEnvelope,
    L2BlockInfo,
);

/// An external handle to communicate with a [InsertTask] spawned
/// in a new thread.
#[derive(Debug)]
pub struct InsertTaskExt {
    /// A receiver channel to receive [InsertTaskOut] events *from* the [InsertTask].
    pub receiver: TokioReceiver<InsertTaskOut>,
    /// A sender channel to send [InsertTaskInput] event *to* the [InsertTask].
    pub sender: TokioSender<InsertTaskInput>,
    /// A join handle to the spawned thread containing the [InsertTask].
    pub handle: JoinHandle<Result<(), InsertTaskError>>,
}

impl InsertTaskExt {
    /// Spawns the [InsertTask] in a new thread, returning an
    /// external-facing wrapper that can be used to communicate with
    /// the spawned task.
    pub fn spawn(
        client: Arc<EngineClient>,
        sync: SyncConfig,
        genesis: B256,
        version: EngineForkchoiceVersion,
        envelope: OpNetworkPayloadEnvelope,
        info: L2BlockInfo,
    ) -> Self {
        let (sender, task_receiver) = tokio::sync::broadcast::channel(1);
        let (task_sender, receiver) = tokio::sync::broadcast::channel(1);
        let input = (task_sender, task_receiver, client, sync, genesis, version, envelope, info);
        let handle = tokio::spawn(async move { InsertTask::execute(input).await });
        Self { receiver, sender, handle }
    }
}

/// The task to insert an unsafe payload into the execution engine.
#[derive(Debug)]
pub struct InsertTask;

impl InsertTask {
    /// Fetches the sync status through the external API.
    pub async fn fetch_sync_status(
        sender: &mut Sender,
        receiver: &mut Receiver,
    ) -> Result<SyncStatus, InsertTaskError> {
        crate::send_until_success!("insert", sender, InsertTaskOut::SyncStatus);
        let response = receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::SyncStatusResponse(response) = response {
            Ok(response)
        } else {
            Err(InsertTaskError::InvalidSyncStatusResponse)
        }
    }

    /// Fetches a state snapshot through the external API.
    pub async fn fetch_state(
        sender: &mut Sender,
        receiver: &mut Receiver,
    ) -> Result<EngineState, InsertTaskError> {
        crate::send_until_success!("insert", sender, InsertTaskOut::StateSnapshot);
        let response = receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::StateResponse(response) = response {
            Ok(*response)
        } else {
            Err(InsertTaskError::InvalidMessageResponse)
        }
    }

    /// Requests an [L2BlockInfo] for the [BlockNumberOrTag::Finalized].
    pub async fn fetch_finalized_info(
        sender: &mut Sender,
        receiver: &mut Receiver,
    ) -> Result<Option<L2BlockInfo>, InsertTaskError> {
        let msg = InsertTaskOut::L2BlockInfo(BlockNumberOrTag::Finalized);
        crate::send_until_success!("insert", sender, msg);
        let response = receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::L2BlockInfoResponse(bi) = response {
            Ok(bi)
        } else {
            Err(InsertTaskError::InvalidMessageResponse)
        }
    }
}

#[async_trait::async_trait]
impl EngineTask for InsertTask {
    type Error = InsertTaskError;
    type Input = Input;

    async fn execute(input: Self::Input) -> Result<(), Self::Error> {
        // Destructure the input.
        let (mut sender, mut receiver, client, config, genesis, version, envelope, block_info) =
            input;

        let sync = Self::fetch_sync_status(&mut sender, &mut receiver).await?;

        // If post finalization EL sync, jump straight to EL sync start
        if config.supports_post_finalization_elsync {
            let msg = InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerStarted);
            crate::send_until_success!("insert", sender, msg);
        } else if sync == SyncStatus::ExecutionLayerWillStart {
            match Self::fetch_finalized_info(&mut sender, &mut receiver).await {
                Ok(Some(bi)) => {
                    // If the genesis is finalized, start EL
                    if bi.block_info.hash == genesis {
                        let msg =
                            InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerStarted);
                        crate::send_until_success!("insert", sender, msg);
                    } else {
                        // If there is a finalized block, finish EL sync.
                        let msg =
                            InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerFinished);
                        crate::send_until_success!("insert", sender, msg);
                    }
                }
                Ok(None) => {
                    // If no block is found, EL started
                    let msg = InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerStarted);
                    crate::send_until_success!("insert", sender, msg);
                }
                Err(_) => {
                    // Temporary derivation error.
                    return Err(InsertTaskError::TemporaryDerivationError);
                }
            }
        }

        let block_root = envelope.parent_beacon_block_root.unwrap_or_default();
        let response = match envelope.payload.clone() {
            ExecutionPayload::V1(payload) => client.new_payload_v1(payload).await,
            ExecutionPayload::V2(payload) => {
                let payload_input = ExecutionPayloadInputV2 {
                    execution_payload: payload.payload_inner,
                    withdrawals: Some(payload.withdrawals),
                };
                client.new_payload_v2(payload_input).await
            }
            ExecutionPayload::V3(payload) => client.new_payload_v3(payload, block_root).await,
        };
        let status = match response {
            Ok(s) => s,
            Err(_) => {
                return Err(InsertTaskError::FailedToInsertNewPayload);
            }
        };

        if status.is_invalid() {
            let msg = InsertTaskOut::InvalidNewPayload;
            crate::send_until_success!("insert", sender, msg);
        }

        // Tell the engine state to update if needed with the new payload.
        let msg = InsertTaskOut::UpdatePayloadStatus(status);
        crate::send_until_success!("insert", sender, msg);

        // Receive response from state.
        let response = receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        match response {
            InsertTaskInput::NewPayloadResponse(true) => {
                info!(target: "insert", "Engine State updated with new payload.");
            }
            InsertTaskInput::NewPayloadResponse(false) => {
                warn!(target: "insert", "Failed to update Engine State with new payload.");
                let msg = InsertTaskOut::FailedToProcessNewPayload;
                crate::send_until_success!("insert", sender, msg);
            }
            _ => {
                return Err(InsertTaskError::InvalidMessageResponse);
            }
        }

        // Fetch the state snapshot.
        let state = Self::fetch_state(&mut sender, &mut receiver).await?;

        // Mark the new payload as valid
        let mut fcu = ForkchoiceState {
            head_block_hash: block_info.block_info.hash,
            safe_block_hash: state.safe_head.block_info.hash,
            finalized_block_hash: state.finalized_head.block_info.hash,
        };

        // Fetch the sync status
        // SEE: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L407-L416
        let sync = Self::fetch_sync_status(&mut sender, &mut receiver).await?;
        if sync == SyncStatus::ExecutionLayerNotFinalized {
            fcu.safe_block_hash = envelope.payload.block_hash();
            fcu.finalized_block_hash = envelope.payload.block_hash();

            // Tell the safe to update the heads given the new payload l2 block ref.
            let msg = InsertTaskOut::NewPayloadNotFinalizedUpdate(block_info);
            crate::send_until_success!("insert", sender, msg);

            // Broadcast cross safe update event.
            let msg = InsertTaskOut::CrossSafeUpdate(block_info);
            crate::send_until_success!("insert", sender, msg);
        }

        // Perform the fork choice update.
        let response = match version {
            EngineForkchoiceVersion::V1 => client.fork_choice_updated_v1(fcu, None).await,
            EngineForkchoiceVersion::V2 => client.fork_choice_updated_v2(fcu, None).await,
            EngineForkchoiceVersion::V3 => client.fork_choice_updated_v3(fcu, None).await,
        };

        let update = match response {
            Ok(u) => u,
            Err(_) => {
                // TODO: reset error for invalid forkchoice state.
                // Otherwise, temporary derivation error.
                return Err(InsertTaskError::TemporaryDerivationError);
            }
        };

        // Send a forkchoice updated status to the engine state.
        // TODO: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L434-L441
        let msg = InsertTaskOut::UpdateForkchoiceState(update.payload_status.clone(), block_info);
        crate::send_until_success!("insert", sender, msg);

        // At this point finish EL sync if not finalized.
        let sync = Self::fetch_sync_status(&mut sender, &mut receiver).await?;
        if sync == SyncStatus::ExecutionLayerNotFinalized {
            let msg = InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerFinished);
            crate::send_until_success!("insert", sender, msg);
            // TODO: add duration to the log.
            info!(target: "insert", "Finished EL sync");
        }

        if update.is_valid() {
            let msg = InsertTaskOut::ForkchoiceUpdated;
            crate::send_until_success!("insert", sender, msg);
        }

        // TODO: make this log more verbose?
        info!(target: "insert", "Inserted new L2 unsafe block (synchronous)");
        debug!(target: "insert", "(unsafe l2 block) Hash {}", envelope.payload.block_hash());
        debug!(target: "insert", "(unsafe l2 block) Number {}", envelope.payload.block_number());

        Ok(())
    }
}
