# [WS] Execution Extension

> Work Stream: Execution Extension support for the `kona-node`.
>

<aside>

**Status - *In Review***

**Author -** @Andreas Bigger

**Reviews - @Benjamin Clabby @Theo Chapuis--Chkaiban @Nick Takacs @Emilia Hane**

**Customer -** RaaS Providers + Chain Operators. Not an ask from Base.

</aside>

## Background

Execution extensions provide a way for arbitrary external services to be installed into a Reth node. To install an exex, Reth provides a context that allows an arbitrary service to interface with the Reth node. Since the `kona-node` is meant to be run alongside `op-reth`, the logical next step to simplify operating the OP Stack is to install the `kona-node` as an EXEX along with other OP Stack components so a single binary is able to run the OP Stack.

## Execution Extension Architecture

Execution extensions should be installed

The [`ExExContext`](https://github.com/paradigmxyz/reth/blob/e12e6c0d04a3e2af8e9ef98923768ea9477f9cba/crates/exex/exex/src/context.rs#L15) provides a way for the execution extension to interface with a reth node.

There are two important parts of the `ExExContext`.

- [`ExExContext::events`](https://github.com/paradigmxyz/reth/blob/e12e6c0d04a3e2af8e9ef98923768ea9477f9cba/crates/exex/exex/src/context.rs#L22-L29) → the execution extensions should send a `FinishedHeight` to notify the reth node that it’s safe to prune the block.
- [`ExExContext::notifications`](https://github.com/paradigmxyz/reth/blob/e12e6c0d04a3e2af8e9ef98923768ea9477f9cba/crates/exex/exex/src/context.rs#L30-L36) → Channel for the execution extension to receive [`ExExNotification`](https://github.com/paradigmxyz/reth/blob/e12e6c0d04a3e2af8e9ef98923768ea9477f9cba/crates/exex/types/src/notification.rs#L10) from the reth node.

    The `ExExNotification` has three variants: when a new `Chain` is committed, when the `Chain` is reorged, and when the chain reverted. The first two must be processed by the execution extension to have an accurate view of the chain.

    - `ExExNotification::ChainCommitted` returns the new `Chain` which the execution extension must process.
    - `ExExNotification::ChainReorged` returns both the old and new `Chain` which the execution extension must process.

    Importantly, notifications can be async iterated over as shown by [this rollup exex example](https://github.com/paradigmxyz/reth-exex-examples/blob/main/rollup/src/main.rs#L62).


## Kona Node RPCs

The Kona Node needs a few RPC URLs in order to pull in needed chain data.

- L1 ETH RPC → This could be for example Reth running Ethereum Mainnet if the `kona-node` instance is syncing OP Mainnet.
- L1 Beacon RPC → This is the L1 consensus (aka beacon) client. Used by Kona’s derivation pipeline to fetch blobs that the OP Stack posts batch submission data to. This could be lighthouse running Ethereum Mainnet.
- L2 Engine RPC →  Used for both the engine RPC methods as well as the L2 execution client’s eth rpc methods. This could be `op-reth` syncing OP Mainnet.

Since we’ll eventually what the `kona-node` installed as an execution extension along with the `kona-batcher`, it is most conveniently installed as an execution extension on top of `op-reth`. This reduces latency between the L2 consensus client and L2 execution client. An alternative option would be to install the `kona-node` as an execution extension on **L1** reth (execution client). This would reduce latency for L1 ETH RPC calls, but not L1 Beacon RPC calls or L2 Engine RPC calls. Additionally, installing the `kona-node` on `op-reth` makes more sense here since the L2 block time is faster than L1. This means there’s more RPC calls that happen between `kona-node` and `op-reth`, not L1 Reth.

## Kona Node Integration

Execution extensions are installed through the Reth node builder. [The rollup exex example](https://github.com/paradigmxyz/reth-exex-examples/blob/main/rollup/src/main.rs#L270) does exactly this.

```rust
reth::cli::Cli::parse_args().run(|builder, _| async move {
    let handle = builder
        .node(EthereumNode::default())
        .install_exex("Rollup", move |ctx| async {
            let connection = Connection::open(DATABASE_PATH)?;

            Ok(Rollup::new(ctx, connection)?.start())
        })
        .launch()
        .await?;

    handle.wait_for_node_exit().await
})
```

This means, in order to install the `kona-node` into `op-reth` as an exex, we’ll need to parse `op-reth` cli flags and build the `op-reth` node as shown above.

There are multiple ways of doing this. A new binary could be introduced to the Kona repository that follows how the exex example constructs and run the execution client with the `kona-node` installed as an exex. Alternatively, the `kona-node` binary could introduce a new `--exex` flag or `exex` subcommand. The subcommand or flag can define it’s entrypoint that uses `op-reth` programmatically to parse the args and install a `kona-node` instance just like how the separate binary would. The benefit of using the new subcommand or flag is that all other `kona-node` cli flags are automatically inherited. Clap (CLI argument parser) may not like this approach though since flags between `op-reth` and `kona-node` may conflict.

Since the `kona-node` will be installed directly into the `op-reth` node, it will need to be use a new L2 provider that buffers the notifications from the exex context, parsing chain state into a buffer that can serve L2 provider rpc methods defined by Kona. A new buffered pipeline will need to be defined that uses the buffered l2 provider.

## Scope

- [ ]  Create a new `exex` subcommand to the `kona-node` or revert to a new binary if flags clash.
- [ ]  Create a new L2 Provider that runs async, listening to exex notifications and buffering chain state for the `kona-node`.
- [ ]  Define a new Buffered Pipeline type that uses the new Buffered L2 Provider.
- [ ]  Construct the `op-reth` node, install the `kona-node` as an exex with the new Buffered L2 Provider + Buffered Pipeline.

## Output

> **6. Too Many Components / Operational Overhead**
>
>
> Managing multiple binaries/clients increases ops burden and mental load.
>
> Cited by:
>
> - Alchemy: Orbit easier because of single client; OP Stack has completely separate binaries.
> - Caldera: OP Stack restarts take longer than Arb; more components to juggle.
> - Pinax: Many chains don’t upgrade at same time; mental burden of versioning.

There has been [clear feedback](https://docs.google.com/document/d/1Kn7V9Zo5vRCs20r4TmG8McZSwKVEZnKS1z_owekWNN0/edit?tab=t.ygbuchsslxzw#heading=h.vtcxbqkb8tv) from RaaS providers and OP Stack rollup node operators. The OP Stack is getting too complex and difficult to operate. One definitive way to improve this experience is to enable chain operators to run a configurable number of OP Stack components within a single binary. The output of this workstream is simply combining the `op-reth` binary with the `kona-node`. In the future, this can be extended to many other components of the OP Stack.
