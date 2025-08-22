# Kona Rollup Binary

Unified OP Stack rollup node combining op-reth and kona-node into a single binary.

## Usage

```bash
# Start with L1 RPC URL
./rollup --l1-rpc-url https://eth-mainnet.alchemy.com/v2/YOUR_KEY

# Start with config file
./rollup --config rollup.toml

# Production deployment
./rollup --chain-id 10 --l1-rpc-url $L1_RPC --data-dir /opt/rollup
```

## Basic Configuration

```toml
# rollup.toml
[general]
chain_id = 10
data_dir = "/opt/rollup/data"

[l1]
rpc_url = "https://eth-mainnet.alchemy.com/v2/YOUR_KEY"

[kona]
buffer_size = 1000
reorg_depth = 64

[reth]
max_peers = 50
```

## Key Features

- **Single Binary**: No separate op-node/op-geth processes
- **Unified Configuration**: Single config file for all settings
- **Built-in Monitoring**: Metrics at `:9090`, health at `:8080`
- **Graceful Shutdown**: Coordinated cleanup on SIGTERM

## Migration from Separate Components

```bash
# 1. Stop existing services
sudo systemctl stop op-node op-geth

# 2. Backup data
sudo cp -r /opt/op-*/data /opt/backup/

# 3. Start unified binary
./rollup --config rollup.toml --data-dir /opt/rollup/data
```

## Documentation

- [Complete guide](../../docs/rollup.md)
- [Config examples](../../docs/config-examples/)
- [Issues](https://github.com/op-rs/kona/issues)