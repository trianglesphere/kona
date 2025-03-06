//! A task for the `engine_forkchoiceUpdated` query.

use op_alloy_provider::ext::engine::OpEngineApi;
use std::sync::Arc;

use tokio::{
    sync::broadcast::{Receiver as TokioReceiver, Sender as TokioSender},
    task::JoinHandle,
};

use crate::{
    EngineClient, EngineState, EngineTask, ForkchoiceTaskError, ForkchoiceTaskInput,
    ForkchoiceTaskOut, SyncStatus,
};

/// The receiver type for the forkchoice task.
type Receiver = TokioReceiver<ForkchoiceTaskInput>;

/// The sender type for the forkchoice task.
type Sender = TokioSender<ForkchoiceTaskOut>;

/// The input type for the [ForkchoiceTask].
type Input = (Sender, Receiver, Arc<EngineClient>);

/// An external handle to communicate with a [ForkchoiceTask] spawned
/// in a new thread.
#[derive(Debug)]
pub struct ForkchoiceTaskExt {
    /// A receiver channel to receive [ForkchoiceTaskOut] events *from* the [ForkchoiceTask].
    pub receiver: TokioReceiver<ForkchoiceTaskOut>,
    /// A sender channel to send [ForkchoiceTaskInput] event *to* the [ForkchoiceTask].
    pub sender: TokioSender<ForkchoiceTaskInput>,
    /// A join handle to the spawned thread containing the [ForkchoiceTask].
    pub handle: JoinHandle<Result<(), ForkchoiceTaskError>>,
}

impl ForkchoiceTaskExt {
    /// Spawns the [ForkchoiceTask] in a new thread, returning an
    /// external-facing wrapper that can be used to communicate with
    /// the spawned task.
    pub fn spawn(client: Arc<EngineClient>) -> Self {
        let (sender, task_receiver) = tokio::sync::broadcast::channel(1);
        let (task_sender, receiver) = tokio::sync::broadcast::channel(1);
        let input = (task_sender, task_receiver, client);
        let handle = tokio::spawn(async move { ForkchoiceTask::execute(input).await });
        Self { receiver, sender, handle }
    }
}

/// The task to update the forkchoice.
#[derive(Debug)]
pub struct ForkchoiceTask;

impl ForkchoiceTask {
    /// Fetches a state snapshot through the external API.
    pub async fn fetch_state(
        sender: &mut Sender,
        receiver: &mut Receiver,
    ) -> Result<EngineState, ForkchoiceTaskError> {
        crate::send_until_success!("fcu", sender, ForkchoiceTaskOut::StateSnapshot);
        let response = receiver.recv().await.map_err(|_| ForkchoiceTaskError::ReceiveError)?;
        if let ForkchoiceTaskInput::StateResponse(response) = response {
            Ok(*response)
        } else {
            Err(ForkchoiceTaskError::InvalidForkchoiceResponse)
        }
    }

    /// Fetches the sync status through the external API.
    pub async fn fetch_sync_status(
        sender: &mut Sender,
        receiver: &mut Receiver,
    ) -> Result<SyncStatus, ForkchoiceTaskError> {
        crate::send_until_success!("fcu", sender, ForkchoiceTaskOut::SyncStatus);
        let response = receiver.recv().await.map_err(|_| ForkchoiceTaskError::ReceiveError)?;
        if let ForkchoiceTaskInput::SyncStatusResponse(response) = response {
            Ok(response)
        } else {
            Err(ForkchoiceTaskError::InvalidSyncStatusResponse)
        }
    }
}

#[async_trait::async_trait]
impl EngineTask for ForkchoiceTask {
    type Error = ForkchoiceTaskError;
    type Input = Input;

    async fn execute(input: Self::Input) -> Result<(), Self::Error> {
        // Destruct the input.
        let (mut sender, mut receiver, client) = input;

        // Check if a forkchoice update is not needed, return early.
        let state = Self::fetch_state(&mut sender, &mut receiver).await?;
        if !state.forkchoice_update_needed {
            return Err(ForkchoiceTaskError::NoForkchoiceUpdateNeeded);
        }

        // If the engine is syncing, log.
        let sync = Self::fetch_sync_status(&mut sender, &mut receiver).await?;
        if sync.is_syncing() {
            warn!(target: "engine", "Attempting to update forkchoice state while EL syncing");
        }

        // Check if the head is behind the finalized head.
        let state = Self::fetch_state(&mut sender, &mut receiver).await?;
        if state.unsafe_head().block_info.number < state.finalized_head().block_info.number {
            return Err(ForkchoiceTaskError::InvalidForkchoiceState(
                state.unsafe_head().block_info.number,
                state.finalized_head().block_info.number,
            ));
        }

        // Send the forkchoice update through the input.
        let forkchoice = state.create_forkchoice_state();
        let update = client
            .fork_choice_updated_v3(forkchoice, None)
            .await
            .map_err(|_| ForkchoiceTaskError::ForkchoiceUpdateFailed)?;

        if update.payload_status.is_valid() {
            let msg = ForkchoiceTaskOut::ForkchoiceUpdated(update);
            crate::send_until_success!("fcu", sender, msg);
        }

        // TODO: The state actor will need to handle this update.
        // TODO: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L360-L363
        crate::send_until_success!("fcu", sender, ForkchoiceTaskOut::UpdateBackupUnsafeHead);
        crate::send_until_success!("fcu", sender, ForkchoiceTaskOut::ForkchoiceNotNeeded);

        Ok(())
    }
}
