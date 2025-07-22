# `kona-node` recipe

> [!WARNING]
>
> `kona-node` is in active development, and this recipe is subject to frequent change (and may not work!) For the time
> being, it is intended to be used for development purposes. Please [file an issue][new-issue] if you have any problems
> during development.

This directory contains a simple `docker-compose` setup for `kona-node` and `op-reth`, including example Grafana
dashboards and a default Prometheus configuration.

By default, this recipe is configured to sync the [`OP Sepolia`][op-sepolia] L2.

## Usage

### Running with Local L1 Nodes (Recommended)

The simplest way to run the complete stack is using the orchestrated startup command that will spin up L1 nodes first, then L2 nodes:

```sh
# Start the full stack: L1 Geth + Lighthouse, then L2 op-reth + kona-node
just up-with-l1

# Shutdown the docker compose environment
just down

# Restart the docker compose environment  
just restart
```

This command will:
1. Start L1 Execution Layer (Geth) and wait for it to be healthy
2. Start L1 Consensus Layer (Lighthouse) and wait for it to be healthy  
3. Start L2 nodes (op-reth and kona-node) along with monitoring services

### Running with External L1 Nodes

If you prefer to use external L1 nodes, you must configure the L1 endpoints in your environment. The `L1_PROVIDER_RPC` and
`L1_BEACON_API` environment variables can be set in [`cfg.env`](./cfg.env).

Once these two environment variables are set, the environment can be spun up and shut down as follows:

```sh
# Start `kona-node`, `op-reth`, and `grafana` + `prometheus` (requires external L1 nodes)
just up

# Shutdown the docker compose environment
just down

# Restart the docker compose environment
just restart
```

### Service Endpoints

When running with `just up-with-l1`, the following services will be available:

- **L1 Geth RPC**: `http://localhost:8546` (HTTP), `http://localhost:8547` (WebSocket)
- **L1 Lighthouse API**: `http://localhost:5052`  
- **L2 op-reth RPC**: `http://localhost:8545`
- **L2 kona-node RPC**: `http://localhost:5060`
- **Grafana Dashboard**: `http://localhost:3000` (admin/admin)
- **Prometheus**: `http://localhost:9090`

### Grafana

The grafana instance can be accessed at `http://localhost:3000` in your browser. The username and password, by default,
are both `admin`.

#### Adding a new visualization

The `kona-node` dashboard is provisioned within the grafana instance by default. A new visualization can be added to the
dashboard by navigating to the `Kona Node` dashboard, and then clicking `Add` > `Visualization` in the top right.

Once your visualization has been added, click `Share` > `Export` (tab), and toggle "Export for sharing externally" on.
Then, copy the JSON, and replace the contents of [`overview.json`](./grafana/dashboards/overview.json)
before making a PR.

## Configuration

### Adjusting host ports

Host ports for L1 nodes, `op-reth` and `kona-node` can be configured in [`cfg.env`](./cfg.env). Note that default ports have been chosen to avoid conflicts:

- L1 Geth: 8546 (HTTP), 8547 (WS), 8552 (Engine), 30304 (P2P)
- L1 Lighthouse: 5052 (HTTP), 9000 (P2P)
- L2 op-reth: 8545 (HTTP), 8551 (Engine), 30303 (P2P)  
- L2 kona-node: 5060 (RPC), 9223 (P2P)

### Syncing a different OP Stack chain

To adjust the chain that the node is syncing, you must modify the `cfg.env` file to specify the desired
network parameters. Specifically:
1. `L1 nodes` (when using `up-with-l1`):
   - Set `L1_NETWORK` to the desired L1 network (e.g., `mainnet`, `sepolia`, `holesky`)
   - Set `L1_CHECKPOINT_SYNC_URL` to the appropriate checkpoint sync URL for the network
1. When using external L1 nodes: Ensure `L1_PROVIDER_RPC` and `L1_BEACON_API` are set to L1 clients that represent the settlement layer of the L2.
1. `op-reth`:
   - Set `L2_CHAIN` to specify the desired L2 chain (e.g., `optimism`, `optimism-sepolia`, `base`, `base-sepolia`)
   - Set `L2_SEQUENCER_HTTP` to specify the sequencer endpoint for the L2 chain
1. `kona-node`:
   - Set `L2_CHAIN_ID` to specify the chain ID of the desired L2 chain

### Adjusting log filters

Log filters can be adjusted by setting the `RUST_LOG` environment variable. This environment variable will be forwarded
to the `kona-node` container's entrypoint.

Example: `export RUST_LOG=engine_builder=trace,runtime=debug`

[op-sepolia]: https://sepolia-optimism.etherscan.io
[op-reth]: https://github.com/paradigmxyz/reth
[new-issue]: https://github.com/op-rs/kona/issues/new
