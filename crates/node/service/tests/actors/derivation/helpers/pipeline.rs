use std::{collections::VecDeque, sync::Arc};

use async_trait::async_trait;
use kona_derive::{
    BuilderError, OriginProvider, Pipeline, PipelineError, PipelineErrorKind, PipelineResult,
    ResetError, Signal, SignalReceiver, StepResult,
};
use kona_genesis::{RollupConfig, SystemConfig};
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use tokio::sync::mpsc;

/// Simple test pipeline that can be used to mock the derivation pipeline.
///
/// It:
/// - checks that l2 safe head cursor's l1 origin is the same as the pipeline origin. If not updates
///   the l1 origin. L1 origin update may temporarily (if the receiver returns `None`) or may detect
///   a reorg.
/// - retrieves the next attributes and checks that the parent is the same as the l2 safe head
///   cursor. Attribute retrieval may fail (if the receiver returns `None`) or may detect a reorg.
/// - returns the attributes
///
/// The pipeline simply streams out all the signals it receives.
pub struct TestPipeline {
    pub l1_block_receiver: mpsc::Receiver<Option<BlockInfo>>,
    pub attributes_receiver: mpsc::Receiver<Option<OpAttributesWithParent>>,

    pub signal_sender: mpsc::Sender<Signal>,

    /// Receives the next system config. The first element is the height of the system config. The
    /// second element is the system config.
    pub system_config: mpsc::Receiver<(u64, SystemConfig)>,

    pub origin: BlockInfo,

    pub attributes_seen: VecDeque<OpAttributesWithParent>,

    pub rollup_config: Arc<RollupConfig>,
}

pub struct PipelineComms {
    pub l1_block_sender: mpsc::Sender<Option<BlockInfo>>,
    pub attributes_sender: mpsc::Sender<Option<OpAttributesWithParent>>,
    pub signal_receiver: mpsc::Receiver<Signal>,
    pub system_config_sender: mpsc::Sender<(u64, SystemConfig)>,
}

impl TestPipeline {
    pub fn new() -> (PipelineComms, Self) {
        let (l1_block_sender, l1_block_receiver) = mpsc::channel(100);
        let (attributes_sender, attributes_receiver) = mpsc::channel(100);
        let (signal_sender, signal_receiver) = mpsc::channel(100);
        let (system_config_sender, system_config_receiver) = mpsc::channel(100);

        let pipeline = Self {
            l1_block_receiver,
            attributes_receiver,
            signal_sender,
            origin: BlockInfo::default(),
            attributes_seen: VecDeque::new(),
            rollup_config: Arc::new(RollupConfig::default()),
            system_config: system_config_receiver,
        };

        (
            PipelineComms {
                l1_block_sender,
                attributes_sender,
                signal_receiver,
                system_config_sender,
            },
            pipeline,
        )
    }

    pub fn with_rollup_config(mut self, rollup_config: RollupConfig) -> Self {
        self.rollup_config = Arc::new(rollup_config);
        self
    }

    pub fn with_origin(mut self, origin: BlockInfo) -> Self {
        self.origin = origin;
        self
    }
}

#[async_trait]
impl SignalReceiver for TestPipeline {
    async fn signal(&mut self, signal: Signal) -> PipelineResult<()> {
        self.signal_sender.send(signal).await.unwrap();
        Ok(())
    }
}

impl Iterator for TestPipeline {
    type Item = OpAttributesWithParent;

    fn next(&mut self) -> Option<Self::Item> {
        self.attributes_seen.pop_front()
    }
}

impl OriginProvider for TestPipeline {
    fn origin(&self) -> Option<BlockInfo> {
        Some(self.origin)
    }
}

#[async_trait]
impl Pipeline for TestPipeline {
    async fn system_config_by_number(
        &mut self,
        expected_number: u64,
    ) -> Result<SystemConfig, PipelineErrorKind> {
        let Some((number, system_config)) = self.system_config.recv().await else {
            return Err(PipelineErrorKind::Critical(PipelineError::Provider(
                "System config pipe broken".to_string(),
            )));
        };

        if number != expected_number {
            return Err(PipelineErrorKind::Critical(PipelineError::Provider(
                "System config number mismatch".to_string(),
            )));
        }

        Ok(system_config)
    }

    fn peek(&self) -> Option<&kona_protocol::OpAttributesWithParent> {
        self.attributes_seen.front()
    }

    fn rollup_config(&self) -> &kona_genesis::RollupConfig {
        &self.rollup_config
    }

    async fn step(&mut self, parent: L2BlockInfo) -> StepResult {
        if parent.l1_origin.number > self.origin.number {
            // Try to advance the origin
            let Some(l1_block) = self.l1_block_receiver.recv().await else {
                return StepResult::OriginAdvanceErr(PipelineErrorKind::Critical(
                    PipelineError::Provider("L1 block channel closed".to_string()),
                ));
            };

            let Some(l1_block) = l1_block else {
                return StepResult::OriginAdvanceErr(PipelineErrorKind::Temporary(
                    PipelineError::NotEnoughData,
                ));
            };

            if self.origin.is_parent_of(&l1_block) {
                return StepResult::OriginAdvanceErr(PipelineErrorKind::Reset(
                    ResetError::ReorgDetected(self.origin.hash, l1_block.parent_hash),
                ));
            }

            self.origin = l1_block;
            return StepResult::AdvancedOrigin;
        }

        let Some(attributes) = self.attributes_receiver.recv().await else {
            return StepResult::StepFailed(PipelineErrorKind::Critical(
                PipelineError::AttributesBuilder(BuilderError::AttributesUnavailable),
            ));
        };

        let Some(attributes) = attributes else {
            return StepResult::StepFailed(PipelineErrorKind::Critical(
                PipelineError::AttributesBuilder(BuilderError::Custom("Builder error".to_string())),
            ));
        };

        if parent.block_info.is_parent_of(&attributes.parent.block_info) {
            return StepResult::StepFailed(PipelineErrorKind::Reset(ResetError::BadParentHash(
                parent.block_info.hash,
                attributes.parent.block_info.hash,
            )));
        }

        self.attributes_seen.push_back(attributes);
        StepResult::PreparedAttributes
    }
}
