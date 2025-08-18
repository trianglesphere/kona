package reorgl1

import (
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	"github.com/op-rs/kona/supervisor/utils"
	"github.com/stretchr/testify/require"
)

type checksFunc func(t devtest.T, sys *presets.SimpleInterop)

func TestL1Reorg(gt *testing.T) {
	// gt.Skip()
	gt.Run("unsafe reorg", func(gt *testing.T) {
		var crossSafeRef, localSafeRef, unsafeRef eth.BlockID
		pre := func(t devtest.T, sys *presets.SimpleInterop) {
			ss := sys.Supervisor.FetchSyncStatus()
			crossSafeRef = ss.Chains[sys.L2ChainA.ChainID()].CrossSafe
			localSafeRef = ss.Chains[sys.L2ChainA.ChainID()].LocalSafe
			unsafeRef = ss.Chains[sys.L2ChainA.ChainID()].LocalUnsafe.ID()
		}
		post := func(t devtest.T, sys *presets.SimpleInterop) {
			require.True(t, sys.L2ELA.IsCanonical(crossSafeRef), "Previous cross-safe block should still be canonical")
			require.True(t, sys.L2ELA.IsCanonical(localSafeRef), "Previous local-safe block should still be canonical")
			require.False(t, sys.L2ELA.IsCanonical(unsafeRef), "Previous unsafe block should have been reorged")
		}
		testL2ReorgAfterL1Reorg(gt, 3, pre, post)
	})
}

func testL2ReorgAfterL1Reorg(gt *testing.T, n int, preChecks, postChecks checksFunc) {
	t := devtest.SerialT(gt)
	ctx := t.Ctx()

	sys := presets.NewSimpleInterop(t)
	trm := utils.NewTestReorgManager(t)

	sys.L1Network.WaitForBlock()

	trm.StopL1CL()

	// sequence a few L1 and L2 blocks
	for range n + 1 {
		trm.GetBlockBuilder().BuildBlock(ctx, nil)

		sys.L2ChainA.WaitForBlock()
		sys.L2ChainA.WaitForBlock()
	}

	// select a divergence block to reorg from
	var divergence eth.L1BlockRef
	{
		tip := sys.L1EL.BlockRefByLabel(eth.Unsafe)
		require.Greater(t, tip.Number, uint64(n), "n is larger than L1 tip, cannot reorg out block number `tip-n`")

		divergence = sys.L1EL.BlockRefByNumber(tip.Number - uint64(n))
	}

	// print the chains before sequencing an alternative L1 block
	sys.L2ChainA.PrintChain()
	sys.L1Network.PrintChain()

	// pre reorg trigger validations and checks
	preChecks(t, sys)

	tipL2_preReorg := sys.L2ELA.BlockRefByLabel(eth.Unsafe)

	// reorg the L1 chain -- sequence an alternative L1 block from divergence block parent
	trm.GetBlockBuilder().BuildBlock(ctx, &divergence.ParentHash)

	// confirm L1 reorged
	sys.L1EL.ReorgTriggered(divergence, 5)
	time.Sleep(5 * time.Second) // wait for L1 reorg to propagate

	// build some blocks
	for range n + 20 {
		trm.GetBlockBuilder().BuildBlock(ctx, nil)

		sys.L2ChainA.WaitForBlock()
		sys.L2ChainA.WaitForBlock()
	}

	// wait until L2 chain A cross-safe ref caught up to where it was before the reorg
	sys.L2CLA.Reached(types.CrossSafe, tipL2_preReorg.Number, 50)
}
