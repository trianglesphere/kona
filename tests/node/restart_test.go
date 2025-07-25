package node

import (
	"sync"
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	"github.com/stretchr/testify/require"
)

// Ensure that kona-nodes restart and sync properly when stopped for a while.
func TestRestartSync(gt *testing.T) {
	t := devtest.SerialT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	t.Gate().Greater(len(nodes), 1, "expected at least two nodes")

	ref := nodes[0]

	var wg sync.WaitGroup
	for _, node := range nodes {
		if node == ref {
			t.Logf("skipping reference node %s", node.Escape().ID().Key())
			continue
		}

		t.Logf("testing restarts for node %s", node.Escape().ID().Key())
		wg.Add(1)
		go func(node *dsl.L2CLNode) {
			defer wg.Done()
			clName := node.Escape().ID().Key()

			require.NoError(t, StopNode(t.Ctx(), node), "failed to stop node %s", clName)
			t.Logf("stopped node %s", clName)

			// Wait for 2 minutes
			time.Sleep(2 * time.Minute)

			require.NoError(t, StartNode(t.Ctx(), node), "failed to start node %s", clName)
			t.Logf("restarted node %s", clName)

			// Check that the node is resyncing with the network
			dsl.CheckAll(t, node.MatchedFn(&ref, types.LocalSafe, 50), node.MatchedFn(&ref, types.LocalUnsafe, 50))

			// Check that the node is connected to the reference node
			peers := node.Peers()
			t.Require().Greater(len(peers.Peers), 0, "expected at least one peer")

			// Check that there is at least a peer with the same ID as the ref node
			found := false
			for _, peer := range peers.Peers {
				if peer.PeerID == ref.PeerInfo().PeerID {
					t.Logf("node %s is connected to reference node %s", clName, ref.Escape().ID().Key())
					found = true
					break
				}
			}

			t.Require().True(found, "expected node %s to be connected to reference node %s", clName, ref.Escape().ID().Key())
		}(&node)
	}
	wg.Wait()
}
