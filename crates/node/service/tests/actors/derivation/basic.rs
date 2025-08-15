//! Basic integration tests for the derivation actor.

use kona_protocol::L2BlockInfo;

use crate::actors::derivation::helpers::TestDerivationBuilder;

#[tokio::test(flavor = "multi_thread")]
async fn test_derivation_actor() -> anyhow::Result<()> {
    kona_cli::init_test_tracing();
    const ROUNDS: u64 = 10;

    let derivation_builder = TestDerivationBuilder::new().valid_rounds(ROUNDS)?;
    let mut derivation = derivation_builder.build();

    derivation.signal_el_sync_complete().unwrap();

    assert!(derivation.current_l1_head().is_none());
    assert_eq!(derivation.current_l2_safe_head(), L2BlockInfo::default());

    for i in 0..ROUNDS {
        let block = derivation_builder.chain_provider.blocks[i as usize].1;

        derivation.update_l1_head(Some(block)).unwrap();

        let l2_block = derivation_builder.l2_chain_provider.blocks[i as usize];

        derivation.update_l2_safe_head(l2_block).unwrap();
    }

    derivation.handle.await.unwrap().unwrap();

    Ok(())
}
