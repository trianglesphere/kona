package presets

import (
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
)
import "github.com/op-rs/kona/supervisor/test_sequencer"

type SimpleInteropForL1Reorg struct {
	*presets.SimpleInterop
	TestSequencer stack.TestSequencer
}

func NewSimpleInteropForL1Reorg(t devtest.T) *SimpleInteropForL1Reorg {
	return &SimpleInteropForL1Reorg{
		SimpleInterop: presets.NewSimpleInterop(t),
		TestSequencer: testsequencer.NewL1TestSequencer(t),
	}
}
