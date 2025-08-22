# Kona Local Providers

This crate provides local buffered provider implementations for caching and reorg handling in the Kona OP Stack implementation.

## Features

- **BufferedL2Provider**: A provider that wraps an underlying Alloy L2 provider with intelligent caching and reorg handling
- **ChainStateBuffer**: LRU cache for managing chain state with reorganization support
- **Chain Event Handling**: Support for processing ExEx notifications for chain events (commits, reorgs, reverts)

## Architecture

The buffered provider sits between the derivation pipeline and the underlying Alloy provider, providing:

1. **Caching Layer**: LRU cache for frequently accessed blocks, headers, receipts, and transactions
2. **Reorg Handling**: Intelligent cache invalidation during chain reorganizations
3. **Event Processing**: Integration with ExEx notifications to maintain cache consistency

## Usage

```rust
use kona_providers_local::{BufferedL2Provider, ChainStateEvent};
use kona_providers_alloy::AlloyL2ChainProvider;
use kona_genesis::RollupConfig;
use std::sync::Arc;

async fn example() -> Result<(), Box<dyn std::error::Error>> {
    // Create an underlying Alloy provider
    let url = "http://localhost:8545".parse()?;
    let alloy_provider = alloy_provider::RootProvider::new_http(url);
    let rollup_config = Arc::new(RollupConfig::default());
    let inner = AlloyL2ChainProvider::new(alloy_provider, rollup_config, 100);
    
    // Wrap with buffered provider
    let provider = BufferedL2Provider::new(inner, 1000, 64);
    
    // Handle chain events from ExEx notifications
    let event = ChainStateEvent::ChainCommitted {
        new_head: alloy_primitives::B256::ZERO,
        committed: vec![],
    };
    provider.handle_chain_event(event).await?;
    
    Ok(())
}
```

## Configuration

- `cache_size`: Number of blocks to cache (affects memory usage)
- `max_reorg_depth`: Maximum reorganization depth to handle before clearing cache

## Provider Traits

The `BufferedL2Provider` implements the following traits from `kona-derive`:

- `ChainProvider`: Basic block and receipt access
- `L2ChainProvider`: L2-specific functionality including system config access
- `BatchValidationProvider`: Batch validation support

## Error Handling

The provider implements comprehensive error handling with automatic fallback to the underlying provider when cache misses occur.