package testsequencer

import (
	"context"
	"fmt"
	"os"
	"strings"

	"github.com/ethereum-optimism/optimism/devnet-sdk/shell/env"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-service/apis"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/ethereum-optimism/optimism/op-test-sequencer/sequencer/seqtypes"
	"github.com/ethereum/go-ethereum/log"
	"github.com/op-rs/kona/supervisor/test_sequencer/builder"
)

type L1TestSequencer struct {
	t          devtest.T
	controller apis.TestSequencerControlAPI
}

func NewL1TestSequencer(t devtest.T) stack.TestSequencer {
	return hydrateL1TestSequencer(t)
}

func (l L1TestSequencer) T() devtest.T {
	return l.t
}

func (l L1TestSequencer) Logger() log.Logger {
	return l.t.Logger()
}

func (l L1TestSequencer) Label(string) string {
	return "l1-test-sequencer"
}

func (l L1TestSequencer) SetLabel(string, string) {
	return
}

func (l L1TestSequencer) ID() stack.TestSequencerID {
	return "l1-test-sequencer"
}

func (l L1TestSequencer) AdminAPI() apis.TestSequencerAdminAPI {
	panic("not implemented")
}

func (l L1TestSequencer) BuildAPI() apis.TestSequencerBuildAPI {
	panic("not implemented")
}

func (l L1TestSequencer) ControlAPI(eth.ChainID) apis.TestSequencerControlAPI {
	return l.controller
}

type l1TestSequencerController struct {
	apis.TestSequencerControlAPI
	opts         *seqtypes.BuildOpts
	blockBuilder builder.L1BlockBuilder
}

func newL1TestSequencerController(blockBuilder builder.L1BlockBuilder) apis.TestSequencerControlAPI {
	return &l1TestSequencerController{blockBuilder: blockBuilder}
}

func (l *l1TestSequencerController) Next(ctx context.Context) error {
	if strings.Contains(l.opts.Parent.String(), "0000000") {
		l.blockBuilder.BuildBlock(ctx, nil)
		return nil
	}
	l.blockBuilder.BuildBlock(ctx, &l.opts.Parent)
	return nil
}

func (l *l1TestSequencerController) New(_ context.Context, opts seqtypes.BuildOpts) error {
	l.opts = &opts
	return nil
}

func hydrateL1TestSequencer(t devtest.T) *L1TestSequencer {
	url := os.Getenv(env.EnvURLVar)
	if url == "" {
		t.Errorf("environment variable %s is not set", env.EnvURLVar)
		return nil
	}

	env, err := env.LoadDevnetFromURL(url)
	if err != nil {
		t.Errorf("failed to load devnet environment from URL %s: %v", url, err)
		return nil
	}

	var engineURL, rpcURL string
	for _, node := range env.Env.L1.Nodes {
		el, ok := node.Services["el"]
		if !ok {
			continue
		}

		engine, ok := el.Endpoints["engine-rpc"]
		if !ok {
			continue
		}

		rpc, ok := el.Endpoints["rpc"]
		if !ok {
			continue
		}

		engineURL = fmt.Sprintf("http://%s:%d", engine.Host, engine.Port)
		rpcURL = fmt.Sprintf("http://%s:%d", rpc.Host, rpc.Port)
		break
	}

	if engineURL == "" || rpcURL == "" {
		t.Errorf("could not find engine or RPC endpoints in the devnet environment")
		return nil
	}

	blockBuilder := builder.NewL1TestBlockBuilder(t, builder.L1BlockBuilderConfig{
		GethRPC:                rpcURL,
		EngineRPC:              engineURL,
		JWTSecret:              env.Env.L1.JWT,
		SafeBlockDistance:      10,
		FinalizedBlockDistance: 20,
	})
	controller := newL1TestSequencerController(blockBuilder)
	return &L1TestSequencer{t, controller}
}
