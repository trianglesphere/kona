//! A task to insert an unsafe payload into the execution engine.

use crate::{
    EngineClient, EngineState, EngineTaskExt, ForkchoiceTask, InsertTaskError, Metrics,
    state::EngineSyncStateUpdate,
};
use alloy_eips::eip7685::EMPTY_REQUESTS_HASH;
use alloy_primitives::FixedBytes;
use alloy_provider::ext::EngineApi;
use alloy_rpc_types_engine::{
    CancunPayloadFields, ExecutionPayloadInputV2, PayloadStatus, PayloadStatusEnum,
    PraguePayloadFields,
};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;
use op_alloy_consensus::OpBlock;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::{
    OpExecutionPayload, OpExecutionPayloadEnvelope, OpExecutionPayloadSidecar,
};
use std::{sync::Arc, time::Instant};

/// The task to insert a payload into the execution engine.
#[derive(Debug)]
pub struct InsertTask {
    /// The engine client.
    client: Arc<EngineClient>,
    /// The rollup config.
    rollup_config: Arc<RollupConfig>,
    /// The network payload envelope.
    envelope: OpExecutionPayloadEnvelope,
    /// If the payload is safe this is true.
    /// A payload is safe if it is derived from a safe block.
    is_payload_safe: bool,
}

impl InsertTask {
    /// Creates a new insert task.
    pub const fn new(
        client: Arc<EngineClient>,
        rollup_config: Arc<RollupConfig>,
        envelope: OpExecutionPayloadEnvelope,
        is_attributes_derived: bool,
    ) -> Self {
        Self { client, rollup_config, envelope, is_payload_safe: is_attributes_derived }
    }

    /// Checks the response of the `engine_newPayload` call.
    const fn check_new_payload_status(status: &PayloadStatusEnum) -> bool {
        matches!(status, PayloadStatusEnum::Valid | PayloadStatusEnum::Syncing)
    }

    fn try_into_block(
        payload: OpExecutionPayload,
        parent_beacon_block_root: FixedBytes<32>,
    ) -> Result<OpBlock, InsertTaskError> {
        match payload {
            payload @ OpExecutionPayload::V1(_) => {
                payload.try_into_block().map_err(InsertTaskError::FromBlockError)
            }
            payload @ OpExecutionPayload::V2(_) => {
                payload.try_into_block().map_err(InsertTaskError::FromBlockError)
            }
            payload @ OpExecutionPayload::V3(_) => payload
                .try_into_block_with_sidecar(&OpExecutionPayloadSidecar::v3(
                    CancunPayloadFields::new(parent_beacon_block_root, vec![]),
                ))
                .map_err(InsertTaskError::FromBlockError),
            payload @ OpExecutionPayload::V4(_) => payload
                .try_into_block_with_sidecar(&OpExecutionPayloadSidecar::v4(
                    CancunPayloadFields::new(parent_beacon_block_root, vec![]),
                    PraguePayloadFields::new(EMPTY_REQUESTS_HASH),
                ))
                .map_err(InsertTaskError::FromBlockError),
        }
    }

    async fn new_payload(
        client: &EngineClient,
        op_payload: OpExecutionPayload,
        parent_beacon_block_root: FixedBytes<32>,
    ) -> Result<PayloadStatus, InsertTaskError> {
        let response = match op_payload {
            OpExecutionPayload::V1(inner) => client.new_payload_v1(inner).await,
            OpExecutionPayload::V2(inner) => {
                let payload_input = ExecutionPayloadInputV2 {
                    execution_payload: inner.payload_inner,
                    withdrawals: Some(inner.withdrawals),
                };

                client.new_payload_v2(payload_input).await
            }
            OpExecutionPayload::V3(inner) => {
                client.new_payload_v3(inner, parent_beacon_block_root).await
            }
            OpExecutionPayload::V4(inner) => {
                client.new_payload_v4(inner, parent_beacon_block_root).await
            }
        }
        .map_err(InsertTaskError::InsertFailed)?;

        if !Self::check_new_payload_status(&response.status) {
            return Err(InsertTaskError::UnexpectedPayloadStatus(response.status));
        }

        Ok(response)
    }
}

#[async_trait]
impl EngineTaskExt for InsertTask {
    type Output = ();

    type Error = InsertTaskError;

    async fn execute(self, state: &mut EngineState) -> Result<(), InsertTaskError> {
        let time_start = Instant::now();

        // Insert the new payload.
        // Form the new unsafe block ref from the execution payload.
        let parent_beacon_block_root = self.envelope.parent_beacon_block_root.unwrap_or_default();
        let insert_time_start = Instant::now();

        let block: OpBlock = Self::try_into_block(self.envelope.payload, parent_beacon_block_root)?;

        let new_unsafe_ref =
            L2BlockInfo::from_block_and_genesis(&block, &self.rollup_config.genesis)
                .map_err(InsertTaskError::L2BlockInfoConstruction)?;

        // TODO(@theochap): we should be able to optimize the `from_block_unchecked` call to remove
        // extra clones and consume the block.
        let (op_payload, _) =
            OpExecutionPayload::from_block_unchecked(new_unsafe_ref.block_info.hash, &block);

        let response =
            Self::new_payload(&self.client, op_payload, parent_beacon_block_root).await?;

        let insert_duration = insert_time_start.elapsed();

        // Send a FCU to canonicalize the imported block.
        ForkchoiceTask::new(
            self.client,
            self.rollup_config,
            EngineSyncStateUpdate {
                cross_unsafe_head: Some(new_unsafe_ref),
                unsafe_head: Some(new_unsafe_ref),
                local_safe_head: self.is_payload_safe.then_some(new_unsafe_ref),
                safe_head: self.is_payload_safe.then_some(new_unsafe_ref),
                ..Default::default()
            },
            None,
        )
        .execute(state)
        .await?;

        let total_duration = time_start.elapsed();

        info!(
            target: "engine",
            response = ?response,
            hash = %new_unsafe_ref.block_info.hash,
            number = new_unsafe_ref.block_info.number,
            total_duration = ?total_duration,
            insert_duration = ?insert_duration,
            "Inserted new unsafe block"
        );

        // Update metrics.
        kona_macros::inc!(counter, Metrics::ENGINE_TASK_COUNT, Metrics::INSERT_TASK_LABEL);

        Ok(())
    }
}
