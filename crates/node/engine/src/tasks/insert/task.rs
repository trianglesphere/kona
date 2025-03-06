//! A task to insert an unsafe payload into the execution engine.

use alloy_eips::BlockNumberOrTag;
use alloy_rpc_types_engine::{
    ExecutionPayloadInputV2, ExecutionPayloadV1, ForkchoiceState, PayloadStatus, PayloadStatusEnum,
};
use kona_protocol::L2BlockInfo;
use kona_rpc::OpAttributesWithParent;
use op_alloy_provider::ext::engine::OpEngineApi;
use std::sync::Arc;

use crate::{
    EngineClient, EngineNewPayloadVersion, EngineState, EngineTask, InsertTaskError,
    InsertTaskInput, InsertTaskOut, SyncStatus,
};

/// The input type for the [InsertTask].
type Input = (Arc<EngineClient>, EngineNewPayloadVersion, OpAttributesWithParent, L2BlockInfo);

/// An external handle to communicate with a [InsertTask] spawned
/// in a new thread.
#[derive(Debug)]
pub struct InsertTaskExt {
    /// A receiver channel to receive [InsertTaskOut] events *from* the [InsertTask].
    pub receiver: tokio::sync::broadcast::Receiver<InsertTaskOut>,
    /// A sender channel to send [InsertTaskInput] event *to* the [InsertTask].
    pub sender: tokio::sync::broadcast::Sender<InsertTaskInput>,
    /// A join handle to the spawned thread containing the [InsertTask].
    pub handle: tokio::task::JoinHandle<Result<(), InsertTaskError>>,
}

impl InsertTaskExt {
    /// Spawns the [InsertTask] in a new thread, returning an
    /// external-facing wrapper that can be used to communicate with
    /// the spawned task.
    pub fn spawn(input: Input) -> Self {
        let (sender, task_receiver) = tokio::sync::broadcast::channel(1);
        let (task_sender, receiver) = tokio::sync::broadcast::channel(1);
        let mut task = InsertTask::new(task_receiver, task_sender);
        let handle = tokio::spawn(async move { task.execute(input).await });
        Self { receiver, sender, handle }
    }
}

/// A strongly typed receiver channel.
type Receiver = tokio::sync::broadcast::Receiver<InsertTaskInput>;

/// A strongly typed sender channel.
type Sender = tokio::sync::broadcast::Sender<InsertTaskOut>;

/// The task to insert an unsafe payload into the execution engine.
#[derive(Debug)]
pub struct InsertTask {
    /// A receiver channel to receive messages from an external actor.
    pub receiver: Receiver,
    /// A sender channel to send messages out to an external actor.
    pub sender: Sender,
}

impl InsertTask {
    /// Creates a new insert task.
    pub const fn new(receiver: Receiver, sender: Sender) -> Self {
        Self { receiver, sender }
    }

    /// Fetches the sync status through the external API.
    pub async fn fetch_sync_status(&mut self) -> Result<SyncStatus, InsertTaskError> {
        crate::send_until_success!("insert", self.sender, InsertTaskOut::SyncStatus);
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::SyncStatusResponse(response) = response {
            Ok(response)
        } else {
            Err(InsertTaskError::InvalidSyncStatusResponse)
        }
    }

    /// Fetches a state snapshot through the external API.
    pub async fn fetch_state(&mut self) -> Result<EngineState, InsertTaskError> {
        crate::send_until_success!("insert", self.sender, InsertTaskOut::StateSnapshot);
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::StateResponse(response) = response {
            Ok(*response)
        } else {
            Err(InsertTaskError::InvalidMessageResponse)
        }
    }

    /// Requests an [L2BlockInfo] for the [BlockNumberOrTag::Finalized].
    pub async fn fetch_finalized_info(&mut self) -> Result<L2BlockInfo, InsertTaskError> {
        let msg = InsertTaskOut::L2BlockInfo(BlockNumberOrTag::Finalized);
        crate::send_until_success!("insert", self.sender, msg);
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
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

    async fn execute(&mut self, input: Self::Input) -> Result<(), Self::Error> {
        // Request the sync status.
        let sync = self.fetch_sync_status().await?;
        if sync == SyncStatus::ExecutionLayerWillStart {
            match self.fetch_finalized_info().await {
                Ok(_) => {
                    // If there is a finalized block, finish EL sync.
                    let msg = InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerFinished);
                    crate::send_until_success!("insert", self.sender, msg);
                }
                Err(_) => {
                    todo!()
                }
            }
        }

        let (client, version, attributes, block_info) = input;
        let response = match version {
            EngineNewPayloadVersion::V2 => {
                let input = ExecutionPayloadInputV2 {
                    execution_payload: ExecutionPayloadV1 {
                        parent_hash: attributes.parent.block_info.hash,
                        fee_recipient: attributes
                            .attributes
                            .payload_attributes
                            .suggested_fee_recipient,
                        state_root: Default::default(),
                        receipts_root: Default::default(),
                        logs_bloom: Default::default(),
                        prev_randao: attributes.attributes.payload_attributes.prev_randao,
                        block_number: block_info.block_info.number,
                        gas_limit: attributes.attributes.gas_limit.unwrap_or(0),
                        gas_used: Default::default(),
                        timestamp: attributes.attributes.payload_attributes.timestamp,
                        extra_data: Default::default(),
                        base_fee_per_gas: Default::default(),
                        block_hash: block_info.block_info.hash,
                        transactions: attributes.attributes.transactions.unwrap_or_default(),
                    },
                    withdrawals: attributes.attributes.payload_attributes.withdrawals,
                };
                client.new_payload_v2(input).await
            }
            EngineNewPayloadVersion::V3 => {
                // client.new_payload_v3(attributes, block_info).await,
                Ok(PayloadStatus { status: PayloadStatusEnum::Valid, latest_valid_hash: None })
            }
            EngineNewPayloadVersion::V4 => {
                // client.new_payload_v4(attributes, block_info).await,
                Ok(PayloadStatus { status: PayloadStatusEnum::Valid, latest_valid_hash: None })
            }
        };
        let status = match response {
            Ok(s) => s,
            Err(_) => {
                return Err(InsertTaskError::FailedToInsertNewPayload);
            }
        };

        if status.is_invalid() {
            let msg = InsertTaskOut::InvalidNewPayload;
            crate::send_until_success!("insert", self.sender, msg);
        }

        // Tell the engine state to update if needed with the new payload.
        let msg = InsertTaskOut::UpdatePayloadStatus(status);
        crate::send_until_success!("insert", self.sender, msg);

        // Receive response from state.
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        match response {
            InsertTaskInput::NewPayloadResponse(true) => {
                info!(target: "insert", "Engine State updated with new payload.");
            }
            InsertTaskInput::NewPayloadResponse(false) => {
                warn!(target: "insert", "Failed to update Engine State with new payload.");
                let msg = InsertTaskOut::FailedToProcessNewPayload;
                crate::send_until_success!("insert", self.sender, msg);
            }
            _ => {
                return Err(InsertTaskError::InvalidMessageResponse);
            }
        }

        // Fetch the state snapshot.
        let state = self.fetch_state().await?;

        // Mark the new payload as valid
        let _fcu = ForkchoiceState {
            head_block_hash: block_info.block_info.hash,
            safe_block_hash: state.safe_head.block_info.hash,
            finalized_block_hash: state.finalized_head.block_info.hash,
        };

        // TODO: In the engine state, process the state update.
        // SEE: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L407-L416

        // TODO: Call FCU
        // TODO: Update sync status
        // TODO: Fire off FCU updated event

        Ok(())
    }
}
