package builder

import (
	"context"
	"encoding/binary"
	"fmt"
	"math/big"
	"math/rand"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-service/client"
	opeth "github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/ethereum-optimism/optimism/op-service/testutils"
	"github.com/ethereum/go-ethereum/beacon/engine"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/crypto"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/ethereum/go-ethereum/ethclient/gethclient"
	"github.com/ethereum/go-ethereum/node"
	"github.com/ethereum/go-ethereum/rpc"
)

type L1BlockBuilder interface {
	BuildBlock(ctx context.Context, parentHash *common.Hash)
}

type L1BlockBuilderConfig struct {
	SafeBlockDistance      uint64
	FinalizedBlockDistance uint64

	GethRPC string

	EngineRPC string
	JWTSecret string
}

type L1TestBlockBuilder struct {
	t                devtest.CommonT
	withdrawalsIndex uint64
	cfg              L1BlockBuilderConfig
	ethClient        *ethclient.Client
	gethClient       *gethclient.Client
	engineClient     client.RPC
}

func NewL1TestBlockBuilder(t devtest.CommonT, cfg L1BlockBuilderConfig) L1BlockBuilder {
	gethClient, err := rpc.Dial(cfg.GethRPC)
	if err != nil {
		t.Errorf("failed to connect to Geth RPC: %v", err)
		t.FailNow()
	}

	opts := []client.RPCOption{
		client.WithGethRPCOptions(rpc.WithHTTPAuth(node.NewJWTAuth(common.HexToHash(cfg.JWTSecret)))),
	}
	engineClient, err := client.NewRPC(t.Ctx(), t.Logger(), cfg.EngineRPC, opts...)
	if err != nil {
		t.Errorf("failed to connect to Geth RPC: %v", err)
		t.FailNow()
	}

	return &L1TestBlockBuilder{
		t,
		1001,
		cfg,
		ethclient.NewClient(gethClient),
		gethclient.New(gethClient),
		engineClient}
}

func (s *L1TestBlockBuilder) rewindTo(ctx context.Context, blockHash common.Hash) (*types.Block, error) {
	s.t.Logf("Rewinding to block %s", blockHash.Hex())

	block, err := s.ethClient.BlockByHash(ctx, blockHash)
	if err != nil {
		s.t.Errorf("failed to fetch block by hash %s: %v", blockHash.Hex(), err)
		return nil, fmt.Errorf("failed to fetch block by hash: %v", err)
	}

	// Attempt rewind using debug_setHead
	err = s.gethClient.SetHead(s.t.Ctx(), block.Number())
	if err != nil {
		s.t.Errorf("failed to rewind to block %s: %v", blockHash.Hex(), err)
		return nil, fmt.Errorf("rewind failed: %v", err)
	}

	// Confirm head matches requested parent
	head, err := s.ethClient.BlockByNumber(ctx, big.NewInt(int64(rpc.LatestBlockNumber)))
	if err != nil {
		s.t.Errorf("failed to fetch latest block: %v", err)
		return nil, fmt.Errorf("failed to fetch latest block: %v", err)
	}

	if head.Hash() != blockHash {
		s.t.Errorf("head mismatch after rewind: expected %s, got %s", blockHash.Hex(), head.Hash().Hex())
		return nil, fmt.Errorf("head mismatch after rewind")
	}

	s.t.Logf("Successfully rewound to block %s", blockHash.Hex())
	return block, nil
}

func (s *L1TestBlockBuilder) BuildBlock(ctx context.Context, parentHash *common.Hash) {
	var head *types.Block
	var err error
	if parentHash != nil {
		head, err = s.rewindTo(ctx, *parentHash)
		if err != nil {
			s.t.Errorf("failed to rewind to parent block: %v", err)
			return
		}
	} else {
		head, err = s.ethClient.BlockByNumber(ctx, big.NewInt(int64(rpc.LatestBlockNumber)))
		if err != nil {
			s.t.Errorf("failed to fetch latest block: %v", err)
			return
		}
	}

	finalizedBlock, _ := s.ethClient.BlockByNumber(ctx, big.NewInt(rpc.FinalizedBlockNumber.Int64()))
	if finalizedBlock == nil {
		// set sb to genesis if safe block is not set
		finalizedBlock, err = s.ethClient.BlockByNumber(ctx, big.NewInt(0))
		if err != nil {
			s.t.Errorf("failed to fetch genesis block: %v", err)
			return
		}
	}

	// progress finalised block
	if head.NumberU64() > s.cfg.FinalizedBlockDistance {
		finalizedBlock, err = s.ethClient.BlockByNumber(ctx, big.NewInt(int64(head.NumberU64()-s.cfg.FinalizedBlockDistance)))
		if err != nil {
			s.t.Errorf("failed to fetch safe block: %v", err)
			return
		}
	}

	safeBlock, _ := s.ethClient.BlockByNumber(ctx, big.NewInt(rpc.SafeBlockNumber.Int64()))
	if safeBlock == nil {
		safeBlock = finalizedBlock
	}

	// progress safe block
	if head.NumberU64() > s.cfg.SafeBlockDistance {
		safeBlock, err = s.ethClient.BlockByNumber(ctx, big.NewInt(int64(head.NumberU64()-s.cfg.SafeBlockDistance)))
		if err != nil {
			s.t.Errorf("failed to fetch safe block: %v", err)
			return
		}
	}

	fcState := engine.ForkchoiceStateV1{
		HeadBlockHash:      head.Hash(),
		SafeBlockHash:      safeBlock.Hash(),
		FinalizedBlockHash: finalizedBlock.Hash(),
	}

	newBlockTimestamp := head.Time() + 6
	nonce := time.Now().UnixNano()
	var nonceBytes [8]byte
	binary.LittleEndian.PutUint64(nonceBytes[:], uint64(nonce))
	randomHash := crypto.Keccak256Hash(nonceBytes[:])
	payloadAttrs := engine.PayloadAttributes{
		Timestamp:             newBlockTimestamp,
		Random:                randomHash,
		SuggestedFeeRecipient: head.Coinbase(),
		Withdrawals:           randomWithdrawals(s.withdrawalsIndex),
		BeaconRoot:            fakeBeaconBlockRoot(head.Time()),
	}

	// Start payload build
	var fcResult engine.ForkChoiceResponse
	err = s.engineClient.CallContext(ctx, &fcResult, "engine_forkchoiceUpdatedV3", []interface{}{fcState, payloadAttrs})
	if err != nil {
		s.t.Errorf("forkchoiceUpdated failed: %v", err)
		return
	}
	if fcResult.PayloadStatus.Status != "VALID" && fcResult.PayloadStatus.Status != "SYNCING" {
		s.t.Errorf("forkchoiceUpdated returned invalid status: %s", fcResult.PayloadStatus.Status)
		return
	}
	if fcResult.PayloadID == nil {
		s.t.Errorf("forkchoiceUpdated did not return a payload ID")
		return
	}

	time.Sleep(150 * time.Millisecond)

	// Get payload
	var envelope engine.ExecutionPayloadEnvelope
	err = s.engineClient.CallContext(ctx, &envelope, "engine_getPayloadV3", []interface{}{fcResult.PayloadID})
	if err != nil {
		s.t.Errorf("getPayload failed: %v", err)
		return
	}
	if envelope.ExecutionPayload == nil {
		s.t.Errorf("getPayload returned empty execution payload")
		return
	}

	blobHashes := make([]common.Hash, 0)
	if envelope.BlobsBundle != nil {
		for _, commitment := range envelope.BlobsBundle.Commitments {
			if len(commitment) != 48 {
				break
			}
			blobHashes = append(blobHashes, opeth.KZGToVersionedHash(*(*[48]byte)(commitment)))
		}
		if len(blobHashes) != len(envelope.BlobsBundle.Commitments) {
			s.t.Errorf("blob hashes length mismatch: expected %d, got %d", len(envelope.BlobsBundle.Commitments), len(blobHashes))
			return
		}
	}

	// Insert
	var npRes engine.PayloadStatusV1
	err = s.engineClient.CallContext(ctx, &npRes, "engine_newPayloadV3",
		[]interface{}{envelope.ExecutionPayload, blobHashes, payloadAttrs.BeaconRoot})
	if err != nil {
		s.t.Errorf("newPayload failed: %v", err)
		return
	}
	if npRes.Status != "VALID" && npRes.Status != "ACCEPTED" {
		s.t.Errorf("newPayload returned invalid status: %s", npRes.Status)
		return
	}

	// Update forkchoice
	updateFc := engine.ForkchoiceStateV1{
		HeadBlockHash:      envelope.ExecutionPayload.BlockHash,
		SafeBlockHash:      safeBlock.Hash(),
		FinalizedBlockHash: finalizedBlock.Hash(),
	}

	err = s.engineClient.CallContext(ctx, &npRes, "engine_forkchoiceUpdatedV3", []interface{}{updateFc, nil})
	if err != nil {
		s.t.Errorf("forkchoiceUpdated failed after newPayload: %v", err)
		return
	}

	s.withdrawalsIndex += uint64(len(envelope.ExecutionPayload.Withdrawals))

	s.t.Logf("Successfully built block %s:%d at timestamp %d",
		envelope.ExecutionPayload.BlockHash.Hex(),
		envelope.ExecutionPayload.Number,
		newBlockTimestamp,
	)
}

func fakeBeaconBlockRoot(time uint64) *common.Hash {
	var dat [8]byte
	binary.LittleEndian.PutUint64(dat[:], time)
	hash := crypto.Keccak256Hash(dat[:])
	return &hash
}

func randomWithdrawals(startIndex uint64) []*types.Withdrawal {
	r := rand.New(rand.NewSource(time.Now().UnixNano()))
	withdrawals := make([]*types.Withdrawal, r.Intn(4))
	for i := 0; i < len(withdrawals); i++ {
		withdrawals[i] = &types.Withdrawal{
			Index:     startIndex + uint64(i),
			Validator: r.Uint64() % 100_000_000, // 100 million fake validators
			Address:   testutils.RandomAddress(r),
			Amount:    uint64(r.Intn(50_000_000_000) + 1),
		}
	}
	return withdrawals
}
