# Confirmation Delayed Provider

The `ConfirmationDelayedProvider` is a wrapper around the standard `RootProvider` that introduces a configurable delay to protect against L1 reorganizations.

## Purpose

This implementation addresses the issue where consuming L1 `RootProvider` responses directly from the tip of the chain can make the system prone to engine resets due to L1 reorgs.

## How it works

- **Block-by-number requests**: Subtract confirmation depth from requested block number
- **Block-by-hash requests**: Pass through unchanged (no reorg risk for specific hashes)
- **Latest block requests**: Return latest block minus confirmation depth

## Configuration

- `--verifier.l1-confs`: Confirmation depth for verifier/validator modes (default: 4)
- `--sequencer.l1-confs`: Confirmation depth for sequencer mode (default: 4)

## Safety

Blocks within the confirmation depth return `None` rather than potentially unstable data.