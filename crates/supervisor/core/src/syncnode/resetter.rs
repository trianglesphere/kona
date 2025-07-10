use super::{ManagedNodeClient, ManagedNodeError};
use alloy_rpc_types_eth::BlockNumHash;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DerivationStorageReader, HeadRefStorageReader};
use kona_supervisor_types::SuperHead;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Debug)]
pub(super) struct Resetter<DB, C> {
    client: Arc<C>,
    db_provider: Arc<DB>,
    reset_guard: Mutex<()>,
}

impl<DB, C> Resetter<DB, C>
where
    DB: HeadRefStorageReader + DerivationStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    /// Creates a new [`Resetter`] with the specified client.
    pub(super) fn new(client: Arc<C>, db_provider: Arc<DB>) -> Self {
        Self { client, db_provider, reset_guard: Mutex::new(()) }
    }

    /// Resets the node using the latest super head.
    pub(crate) async fn reset(&self) -> Result<(), ManagedNodeError> {
        let _guard = self.reset_guard.lock().await;

        info!(target: "resetter", "Resetting the node");
        let local_safe = self.get_latest_valid_local_safe().await?;
        let local_unsafe = local_safe.clone();

        let SuperHead { mut cross_unsafe, mut cross_safe, mut finalized, .. } =
            self.db_provider.get_super_head().inspect_err(|err| {
                error!(target: "resetter", %err, "Failed to get super head");
            })?;

        for ref_block in [&mut finalized, &mut cross_safe, &mut cross_unsafe] {
            if ref_block.number > local_safe.number {
                *ref_block = local_safe;
            }
        }

        info!(target: "resetter",
            %local_unsafe,
            %cross_unsafe,
            %local_safe,
            %cross_safe,
            %finalized,
            "Resetting managed node with latest information",
        );

        self.client
            .reset(
                local_unsafe.id(),
                cross_unsafe.id(),
                local_safe.id(),
                cross_safe.id(),
                finalized.id(),
            )
            .await
            .inspect_err(|err| {
                error!(target: "resetter", %err, "Failed to reset managed node");
            })?;

        Ok(())
    }

    async fn get_latest_valid_local_safe(&self) -> Result<BlockInfo, ManagedNodeError> {
        let derivation_state = self.db_provider.latest_derivation_state()?;

        let mut local_safe = derivation_state.derived;
        let mut source_block = derivation_state.source;

        loop {
            let node_block = self.client.block_ref_by_number(local_safe.number).await.inspect_err(
                |err| error!(target: "resetter", %err, "Failed to get block by number"),
            )?;

            if node_block == local_safe {
                return Ok(local_safe);
            }

            if source_block.number == 0 {
                error!(target: "resetter", "Source block number is 0, cannot reset further");
                return Err(ManagedNodeError::ResetFailed);
            }

            // Get the previous source block id
            let prev_source_block_id =
                BlockNumHash { number: source_block.number - 1, hash: source_block.parent_hash };

            // Get the latest derived block at the previous source block
            local_safe = self
                .db_provider
                .latest_derived_block_at_source(prev_source_block_id)
                .map_err(|err| {
                    error!(target: "resetter", %err, "Failed to get latest derived block for the previous source block");
                    ManagedNodeError::ResetFailed
                })?;

            // Get the actual source block for the current local safe
            // this helps to skip the empty source block
            source_block = self
                .db_provider
                .derived_to_source(local_safe.id())
                .map_err(|err| {
                    error!(target: "resetter", %err, "Failed to get source block for the local safe head ref");
                    ManagedNodeError::ResetFailed
                })?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncnode::{AuthenticationError, ManagedNodeError};
    use alloy_eips::BlockNumHash;
    use alloy_primitives::{B256, ChainId};
    use async_trait::async_trait;
    use jsonrpsee::core::client::Subscription;
    use kona_interop::{DerivedRefPair, SafetyLevel};
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{DerivationStorageReader, HeadRefStorageReader, StorageError};
    use kona_supervisor_types::{OutputV0, Receipts, SubscriptionEvent, SuperHead};
    use mockall::{mock, predicate};

    // Mock for HeadRefStorageReader
    mock! {
        #[derive(Debug)]
        pub Db {}

        impl HeadRefStorageReader for Db {
            fn get_safety_head_ref(&self, level: SafetyLevel) -> Result<BlockInfo, StorageError>;
            fn get_super_head(&self) -> Result<SuperHead, StorageError>;
        }

        impl DerivationStorageReader for Db {
            fn derived_to_source(&self, derived_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derived_block_at_source(&self, source_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derivation_state(&self) -> Result<DerivedRefPair, StorageError>;
        }
    }

    mock! {
        #[derive(Debug)]
        pub Client {}

        #[async_trait]
        impl ManagedNodeClient for Client {
            async fn chain_id(&self) -> Result<ChainId, ManagedNodeError>;
            async fn subscribe_events(&self) -> Result<Subscription<SubscriptionEvent>, ManagedNodeError>;
            async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ManagedNodeError>;
            async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ManagedNodeError>;
            async fn pending_output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ManagedNodeError>;
            async fn l2_block_ref_by_timestamp(&self, timestamp: u64) -> Result<BlockInfo, ManagedNodeError>;
            async fn block_ref_by_number(&self, block_number: u64) -> Result<BlockInfo, ManagedNodeError>;
            async fn reset(&self, unsafe_id: BlockNumHash, cross_unsafe_id: BlockNumHash, local_safe_id: BlockNumHash, cross_safe_id: BlockNumHash, finalised_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn provide_l1(&self, block_info: BlockInfo) -> Result<(), ManagedNodeError>;
            async fn update_finalized(&self, finalized_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn update_cross_unsafe(&self, cross_unsafe_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn update_cross_safe(&self, source_block_id: BlockNumHash, derived_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
        }
    }

    fn make_super_head() -> SuperHead {
        SuperHead {
            local_unsafe: BlockInfo::new(B256::from([0u8; 32]), 5, B256::ZERO, 0),
            cross_unsafe: BlockInfo::new(B256::from([1u8; 32]), 4, B256::ZERO, 0),
            local_safe: BlockInfo::new(B256::from([2u8; 32]), 3, B256::ZERO, 0),
            cross_safe: BlockInfo::new(B256::from([3u8; 32]), 2, B256::ZERO, 0),
            finalized: BlockInfo::new(B256::from([4u8; 32]), 1, B256::ZERO, 0),
            l1_source: BlockInfo::new(B256::from([54u8; 32]), 100, B256::ZERO, 0),
        }
    }

    #[tokio::test]
    async fn test_reset_success() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_latest_derivation_state().returning(move || {
            Ok(DerivedRefPair {
                derived: super_head.local_safe.clone(),
                source: super_head.l1_source.clone(),
            })
        });
        db.expect_get_super_head().returning(move || Ok(super_head));

        let mut client = MockClient::new();
        client.expect_block_ref_by_number().returning(move |_| Ok(super_head.local_safe));

        client.expect_reset().returning(|_, _, _, _, _| Ok(()));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_ok());
    }

    #[tokio::test]
    async fn test_reset_latest_derivation_error() {
        let mut db = MockDb::new();
        db.expect_latest_derivation_state().returning(|| Err(StorageError::DatabaseNotInitialised));

        let client = MockClient::new();

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_err());
    }

    #[tokio::test]
    async fn test_reset_super_head_error() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_latest_derivation_state().returning(move || {
            Ok(DerivedRefPair {
                derived: super_head.local_safe.clone(),
                source: super_head.l1_source.clone(),
            })
        });
        db.expect_get_super_head().returning(|| Err(StorageError::DatabaseNotInitialised));

        let mut client = MockClient::new();
        client.expect_block_ref_by_number().returning(move |_| Ok(super_head.local_safe));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_err());
    }

    #[tokio::test]
    async fn test_reset_block_error() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_latest_derivation_state().returning(move || {
            Ok(DerivedRefPair {
                derived: super_head.local_safe.clone(),
                source: super_head.l1_source.clone(),
            })
        });
        db.expect_get_super_head().returning(move || Ok(super_head));

        let mut client = MockClient::new();
        client.expect_block_ref_by_number().returning(|_| {
            Err(ManagedNodeError::Authentication(AuthenticationError::InvalidHeader))
        });

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_err());
    }

    #[tokio::test]
    async fn test_reset_inconsistency() {
        let super_head = make_super_head();
        let prev_source_block = BlockInfo::new(B256::from([8u8; 32]), 101, B256::ZERO, 0);
        let last_valid_derived_block = BlockInfo::new(B256::from([6u8; 32]), 9, B256::ZERO, 0);
        let current_source_block =
            BlockInfo::new(B256::from([7u8; 32]), 102, prev_source_block.hash, 0);

        let mut db = MockDb::new();
        db.expect_latest_derivation_state().returning(move || {
            Ok(DerivedRefPair {
                derived: super_head.local_safe.clone(),
                source: current_source_block.clone(),
            })
        });
        db.expect_get_super_head().returning(move || Ok(super_head));

        let mut client = MockClient::new();
        // Return a block that does not match local_safe
        client
            .expect_block_ref_by_number()
            .with(predicate::eq(super_head.local_safe.number))
            .returning(|_| Ok(BlockInfo::new(B256::from([4u8; 32]), 3, B256::ZERO, 0)));

        // return expected values when get_last_valid_derived_block() is called
        db.expect_latest_derived_block_at_source()
            .with(predicate::eq(prev_source_block.id()))
            .returning(move |_| Ok(last_valid_derived_block));
        db.expect_derived_to_source()
            .with(predicate::eq(last_valid_derived_block.id()))
            .returning(move |_| Ok(current_source_block));

        // On second call, return the last valid derived block
        client
            .expect_block_ref_by_number()
            .with(predicate::eq(last_valid_derived_block.number))
            .returning(move |_| Ok(last_valid_derived_block));
        client.expect_reset().times(1).returning(|_, _, _, _, _| Ok(()));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_ok());
    }

    #[tokio::test]
    async fn test_reset_rpc_error() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_latest_derivation_state().returning(move || {
            Ok(DerivedRefPair {
                derived: super_head.local_safe.clone(),
                source: super_head.l1_source.clone(),
            })
        });
        db.expect_get_super_head().returning(move || Ok(super_head));

        let mut client = MockClient::new();
        client.expect_block_ref_by_number().returning(move |_| Ok(super_head.local_safe));
        client.expect_reset().returning(|_, _, _, _, _| {
            Err(ManagedNodeError::Authentication(AuthenticationError::InvalidJwt))
        });

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_err());
    }
}
