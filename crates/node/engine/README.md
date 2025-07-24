# `kona-engine`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-engine"><img src="https://img.shields.io/crates/v/kona-engine.svg?label=kona-engine&labelColor=2a2f35" alt="Kona Engine"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
<a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>

An extensible implementation of the [OP Stack][op-stack] rollup node engine client that provides a robust task-based execution system for managing L2 blockchain operations.

## Overview

The `kona-engine` crate implements the execution engine component of an OP Stack rollup node. It provides a task queue-based architecture for managing blockchain operations like block building, payload insertion, forkchoice updates, and state consolidation. The engine coordinates with both L1 and L2 blockchain networks through authenticated RPC connections and maintains synchronized blockchain state.

## How the Engine Works

The engine operates as a priority-based task queue system where different operations are represented as `EngineTask` variants. These tasks are executed in a specific priority order to ensure proper blockchain synchronization and maintain consistency with the [OP Stack derivation specification][derivation-spec].

### Core Components

1. **Engine Task Queue**: The central `Engine` struct manages a priority queue of `EngineTask` operations
2. **Engine State**: Tracks the current synchronization state including unsafe, safe, and finalized heads
3. **Engine Client**: Provides authenticated RPC communication with execution layer (EL) and L1/L2 providers
4. **Task System**: Implements different task types with specific error handling and retry logic

### Task Priority System

Tasks are executed in the following priority order (highest to lowest):

1. **ForkchoiceUpdate** - Synchronizes the engine's view of the canonical chain
2. **Build** - Creates new blocks with given payload attributes (sequencer priority)
3. **Insert** - Imports unsafe blocks from gossip network
4. **Consolidate** - Advances the safe chain through L1 derivation
5. **Finalize** - Finalizes L2 blocks based on L1 finalization

This ordering ensures that:
- Outstanding forkchoice updates are processed immediately
- Block building (sequencing) takes priority over other operations  
- Unsafe block gossip is imported promptly
- Safe chain advancement happens at appropriate times

### State Management

The engine maintains several blockchain head pointers:

- **Unsafe Head**: Most recent block from P2P network (may be invalid)
- **Cross-Unsafe Head**: Cross-verified unsafe head (pre-interop: same as unsafe)
- **Local Safe Head**: Derived from L1, completed span-batch (not cross-verified)
- **Safe Head**: L1-derived and cross-verified safe dependencies
- **Finalized Head**: Derived from finalized L1 data with only finalized dependencies

### Error Handling

The engine implements a sophisticated error handling system with four severity levels:

- **Temporary**: Errors that should be retried (network issues, temporary EL unavailability)
- **Critical**: Errors that should be propagated to the engine actor (permanent failures)
- **Reset**: Errors requiring engine reset to find new sync starting point
- **Flush**: Errors requiring derivation pipeline flush

## Crate Structure

```
src/
├── lib.rs                 # Public API exports and crate documentation
├── task_queue/            # Core engine and task execution system
│   ├── core.rs           # Main Engine struct with task queue processing
│   └── tasks/            # Individual task implementations
│       ├── task.rs       # EngineTask enum and execution logic
│       ├── forkchoice/   # Forkchoice update task
│       ├── build/        # Block building task
│       ├── insert/       # Unsafe block insertion task
│       ├── consolidate/  # Safe chain consolidation task
│       └── finalize/     # Block finalization task
├── state/                # Engine state management
│   └── core.rs          # EngineState and sync state structures
├── client.rs             # Engine API client with JWT authentication
├── attributes.rs         # Payload attributes matching utilities
├── versions.rs           # Engine API version selection logic
├── kinds.rs              # Engine kind definitions
├── query.rs              # Engine query system for external communication
└── metrics/              # Metrics collection and reporting
    └── mod.rs           # Metrics definitions and instrumentation
```

## Task Logic

### Engine Task Execution

The engine processes tasks through the following flow:

1. **Task Enqueuing**: External actors enqueue `EngineTask` variants into the priority queue
2. **Task Draining**: The `Engine::drain()` method processes tasks in priority order
3. **Task Execution**: Each task executes atomically with exclusive access to `EngineState`
4. **Error Handling**: Failed tasks remain in queue for retry based on error severity
5. **State Updates**: Successful tasks update engine state and notify subscribers

### Task Retry Logic

Tasks implement sophisticated retry mechanisms:

- **Temporary errors**: Task retries immediately in a loop until success or severity change
- **Critical errors**: Task fails immediately and error propagates to engine actor
- **Reset/Flush errors**: Task fails and triggers engine-level recovery procedures

### Engine Reset Process

When reset is required, the engine:

1. Clears all pending tasks from the queue
2. Uses `find_starting_forkchoice` to locate a valid sync starting point
3. Applies a forkchoice update to reorg the execution layer to the starting point
4. Reconstructs system configuration for the new safe head
5. Resumes normal operation from the new starting point

## Error Cases and Contexts

### Common Error Scenarios

1. **Network/RPC Errors** (Temporary)
   - Connection timeouts to L1/L2 providers
   - JWT authentication failures  
   - Temporary EL unavailability
   - **Context**: Network instability, node restarts, configuration issues

2. **Invalid Payloads** (Critical)
   - Malformed execution payloads
   - Invalid block hashes or state roots
   - Consensus rule violations
   - **Context**: Corrupted data, implementation bugs, chain configuration mismatches

3. **Sync Conflicts** (Reset)
   - Irreconcilable fork choice conflicts
   - Missing parent blocks in derivation
   - L1 reorgs affecting safe chain
   - **Context**: Deep L1 reorgs, derivation pipeline issues, corrupted local state

4. **Derivation Errors** (Flush)
   - Invalid span batches requiring pipeline flush
   - Sequencer batch validation failures
   - Channel timeout violations
   - **Context**: Invalid sequencer data, L1 congestion, protocol violations

### Error Recovery

The engine provides multiple levels of error recovery:

- **Task-level**: Individual task retry with exponential backoff
- **Engine-level**: Reset to find new sync starting point when state becomes inconsistent  
- **System-level**: Integration with external actors for pipeline flushing and recovery

<!-- Hyper Links -->

[op-stack]: https://specs.optimism.io
[derivation-spec]: https://specs.optimism.io/protocol/derivation.html
