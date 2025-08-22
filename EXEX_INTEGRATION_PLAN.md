# KonaNode ExEx Integration Plan

## Overview
Integrate kona-node as an execution extension (ExEx) within op-reth by refactoring CLI components and creating proper ExEx lifecycle management.

## Architecture

### 1. CLI Refactoring
**Target**: `crates/utilities/cli`

#### New Module: `crates/utilities/cli/src/node_config.rs`
```rust
pub struct NodeCliConfig {
    pub mode: NodeMode,
    pub l1_eth_rpc: Url,
    pub l1_trust_rpc: bool,
    pub l1_beacon: Url,
    pub l2_trust_rpc: bool,
    pub l2_config_file: Option<PathBuf>,
    pub p2p: P2PConfig,
    pub rpc: RpcConfig,
    pub sequencer: SequencerConfig,
}
```

#### Refactored Components
- **P2PConfig**: Extract from `bin/node/src/flags/p2p.rs`
- **RpcConfig**: Extract from `bin/node/src/flags/rpc.rs`
- **SequencerConfig**: Extract from `bin/node/src/flags/sequencer.rs`
- **GlobalConfig**: Extract chain ID and override logic

### 2. ExEx Implementation
**Location**: `bin/rollup/src/exex.rs`

#### Core Structure
```rust
use tokio_util::sync::CancellationToken;

pub struct KonaNodeExEx<Node> {
    ctx: ExExContext<Node>,
    provider: BufferedL2Provider,
    actor_handles: Vec<tokio::task::JoinHandle<()>>,
    cancellation: CancellationToken,
    config: NodeCliConfig,
}

#[derive(Debug, thiserror::Error)]
pub enum KonaExExError {
    #[error("Node service error: {0}")]
    NodeService(String),
    #[error("Provider error: {0}")]
    Provider(#[from] kona_providers_local::BufferedProviderError),
    #[error("ExEx context error: {0}")]
    Context(String),
    #[error("Actor system error: {0}")]
    ActorSystem(String),
}
```

#### Key Methods
- `new()`: Initialize from CLI args and ExEx context
- `start()`: Launch kona-node service with buffered provider
- `handle_notification()`: Process chain events from op-reth
- `sync_state()`: Coordinate state between op-reth and kona-node

### 3. CLI Integration
**Location**: `bin/rollup/src/cli.rs`

#### CLI Parser Extension
```rust
use clap::{Args, Parser};

#[derive(Parser)]
struct RollupCli {
    #[command(flatten)]
    reth: reth::cli::Cli,
    
    #[command(flatten)]
    kona: KonaNodeArgs,
}

#[derive(Args)]
struct KonaNodeArgs {
    /// Kona node operation mode
    #[arg(long = "kona.mode", default_value = "validator")]
    mode: NodeMode,
    
    /// L1 beacon API URL (required for kona-node)
    #[arg(long = "kona.l1-beacon")]
    l1_beacon: Url,
    
    /// Custom rollup config file (overrides registry)
    #[arg(long = "kona.config-file")]
    config_file: Option<PathBuf>,
    
    /// P2P configuration (all flags prefixed with p2p.*)
    #[command(flatten)]
    p2p: P2PArgs,
    
    /// RPC configuration for kona (prefixed with kona.rpc.*)
    #[command(flatten, next_help_heading = "Kona RPC")]
    rpc: KonaRpcArgs,
    
    /// Sequencer configuration
    #[command(flatten)]
    sequencer: SequencerArgs,
}

// Separate RPC args to avoid conflicts with reth RPC
#[derive(Args)]
struct KonaRpcArgs {
    #[arg(long = "kona.rpc.disabled", default_value = "false")]
    disabled: bool,
    
    #[arg(long = "kona.rpc.port", default_value = "9546")]
    port: u16,
    
    // ... other RPC args with kona.rpc prefix
}
```

### 4. Provider Integration
**Location**: `crates/providers/local/src/buffered.rs`

#### L2 Provider Bridge
- Replace kona-node's L2 engine RPC with BufferedL2Provider
- Map ExEx notifications to provider state updates
- Handle reorgs through buffer invalidation

### 5. Service Orchestration
**Location**: `crates/rollup/service/src/exex.rs`

#### Lifecycle Management
1. **Initialization**: Parse CLI → Build RollupConfig → Create JWT secret
2. **Service Creation**: Build RollupNode with BufferedL2Provider
3. **Event Loop**: Process ExEx notifications → Update kona-node state
4. **Shutdown**: Graceful termination on ExEx stop

#### Actor System Management
```rust
impl KonaNodeExEx<Node> {
    async fn start_actors(&mut self) -> Result<(), KonaExExError> {
        // Start all kona-node actors
        let derivation_handle = tokio::spawn({
            let cancel = self.cancellation.clone();
            async move {
                tokio::select! {
                    _ = cancel.cancelled() => {},
                    _ = derivation_actor.run() => {},
                }
            }
        });
        
        let engine_handle = tokio::spawn({
            let cancel = self.cancellation.clone();
            async move {
                tokio::select! {
                    _ = cancel.cancelled() => {},
                    _ = engine_actor.run() => {},
                }
            }
        });
        
        // Store handles for cleanup
        self.actor_handles.push(derivation_handle);
        self.actor_handles.push(engine_handle);
        // ... other actors
        
        Ok(())
    }
    
    async fn shutdown(&mut self) {
        // Signal all actors to stop
        self.cancellation.cancel();
        
        // Wait for graceful shutdown
        for handle in self.actor_handles.drain(..) {
            let _ = tokio::time::timeout(
                Duration::from_secs(10),
                handle
            ).await;
        }
    }
}
```

## Implementation Steps

### Phase 1: CLI Refactoring
1. Create `NodeCliConfig` struct in `kona-cli`
2. Extract P2P, RPC, Sequencer configs to `kona-cli`
3. Create builder pattern for config composition

### Phase 2: ExEx Core
1. Implement `KonaNodeExEx` with proper trait bounds
2. Create notification handler mapping
3. Integrate BufferedL2Provider as L2 source

### Phase 3: Service Integration
1. Wire RollupNode to use BufferedL2Provider
2. Implement state synchronization logic
3. Add metrics and logging

### Phase 4: CLI Parser
1. Extend reth CLI with kona args
2. Validate arg combinations
3. Create config from parsed args

## Technical Details

### Provider Mapping
- **L1 Provider**: Use reth's existing L1 provider from ExEx context
- **L2 Provider**: BufferedL2Provider wraps ExEx notifications (no engine RPC needed)
- **Beacon Client**: Created from kona CLI args (required for blob fetching)
- **JWT Secret**: Not needed - ExEx runs in-process with op-reth

### Critical Integration Points
1. **No L2 Engine RPC**: Since kona runs as ExEx, it doesn't need separate L2 engine connection
2. **Provider Types**: Use concrete types from kona-providers-alloy:
   ```rust
   type L1Provider = AlloyChainProvider<RootProvider<Optimism>>;
   type L2Provider = BufferedL2Provider;
   ```
3. **Runtime Sharing**: ExEx shares op-reth's tokio runtime - no separate runtime needed

### State Synchronization
```rust
use reth_exex::{ExExContext, ExExNotification};
use kona_providers_local::ChainStateEvent;

// Convert reth notifications to kona events
impl From<ExExNotification> for ChainStateEvent {
    fn from(notification: ExExNotification) -> Self {
        match notification {
            ExExNotification::ChainCommitted { new_tip } => {
                ChainStateEvent::ChainCommitted { 
                    new_head: new_tip.tip().header.hash,
                    committed: new_tip.blocks().to_vec(),
                }
            }
            ExExNotification::ChainReorged { old, new } => {
                ChainStateEvent::ChainReorged {
                    old_blocks: old.into_iter().collect(),
                    new_blocks: new.into_iter().collect(),
                }
            }
            ExExNotification::ChainReverted { old } => {
                ChainStateEvent::ChainReverted {
                    blocks: old.into_iter().collect(),
                }
            }
        }
    }
}

// Handle notifications
async fn handle_notification(&mut self, notification: ExExNotification) {
    let event: ChainStateEvent = notification.into();
    self.provider.handle_chain_event(event).await;
}
```

### Error Handling
- Propagate kona-node errors to ExEx
- Log and recover from transient failures
- Graceful degradation on P2P issues

## Dependencies
```toml
[dependencies]
# Reth
reth = { git = "https://github.com/paradigmxyz/reth", branch = "main" }
reth-exex = { git = "https://github.com/paradigmxyz/reth", branch = "main" }
reth-node-optimism = { git = "https://github.com/paradigmxyz/reth", branch = "main" }

# Kona
kona-node-service = { path = "../../crates/node/service" }
kona-providers-local = { path = "../../crates/providers/local" }
kona-cli = { path = "../../crates/utilities/cli" }
```

## Validation
1. Unit tests for config parsing
2. Integration tests for ExEx lifecycle
3. E2E test with op-reth node
4. Performance benchmarks for notification processing