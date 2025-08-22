# Kona Rollup Guide

Complete guide for the unified Kona rollup binary.

## Quick Start

### 1. Basic Setup
```bash
# Start with minimal config
./rollup --l1-rpc-url https://eth-mainnet.alchemy.com/v2/YOUR_KEY

# Create config file
./rollup generate --chain optimism --output ./config

# Start with config
./rollup --config ./config/rollup.toml
```

### 2. Environment Variables
```bash
export ROLLUP_L1_RPC_URL="https://eth-mainnet.alchemy.com/v2/YOUR_KEY"
export ROLLUP_DATA_DIR="/opt/rollup/data" 
export ROLLUP_CHAIN_ID="10"
```

## Configuration

### Essential Config File
```toml
# rollup.toml
[general]
chain_id = 10              # 10=OP Mainnet, 8453=Base, 420=OP Goerli
data_dir = "/opt/rollup/data"
log_level = "info"

[l1]
rpc_url = "https://eth-mainnet.alchemy.com/v2/YOUR_KEY"
beacon_url = "https://beacon-api.example.com"  # Optional but recommended

[l2]  
rpc_url = "https://mainnet.optimism.io"        # Optional for validation

[kona]
buffer_size = 1000         # Cache size (blocks)
reorg_depth = 64          # Reorg safety margin
max_concurrent_requests = 10

[reth]
max_peers = 50
discovery_port = 30303
engine_http_port = 8551

[metrics]
enabled = true
port = 9090
```

### Command Line Options
```bash
# Core options
--config <file>           Config file path
--chain-id <id>          Chain ID (10, 8453, 420)
--l1-rpc-url <url>       L1 RPC endpoint (required)
--data-dir <path>        Data directory
--log-level <level>      debug|info|warn|error

# Network options
--http-port <port>       HTTP RPC port (8545)
--p2p-port <port>        P2P port (30303)
--metrics-port <port>    Metrics port (9090)

# Component-specific options
--kona-buffer-size <n>   ExEx buffer size
--reth-max-peers <n>     Max peer connections
```

## Common Deployment Scenarios

### Production Optimism Node
```bash
./rollup \
  --chain-id 10 \
  --l1-rpc-url $L1_RPC_URL \
  --l1-beacon-url $BEACON_URL \
  --data-dir /var/lib/kona \
  --http-port 8545 \
  --config production.toml
```

### Base Mainnet Node
```bash
./rollup \
  --chain-id 8453 \
  --l1-rpc-url $L1_RPC_URL \
  --data-dir /var/lib/kona-base \
  --config base.toml
```

### Development Setup
```bash
./rollup \
  --chain-id 420 \
  --l1-rpc-url $GOERLI_RPC_URL \
  --data-dir ./dev-data \
  --log-level debug
```

## Migration from Separate Components

### Quick Migration
```bash
# 1. Stop old services
sudo systemctl stop op-node op-geth

# 2. Backup data
sudo cp -r /opt/op-geth/data /opt/backup/
sudo cp -r /opt/op-node/data /opt/backup/

# 3. Create config (maps old flags to new format)
./rollup generate --chain optimism --output /opt/rollup

# 4. Start unified service  
./rollup --config /opt/rollup/rollup.toml --data-dir /opt/rollup/data
```

### Port Mapping
| Old Component | Old Port | New Config |
|---------------|----------|------------|
| op-geth HTTP | 8545 | reth.engine_http_port = 8551 |
| op-geth P2P | 30303 | reth.discovery_port = 30303 |
| op-node P2P | 9222 | Not needed (integrated) |

### Systemd Service
```ini
# /etc/systemd/system/rollup.service
[Unit]
Description=Kona Rollup Node
After=network.target

[Service]
Type=simple
User=rollup
ExecStart=/usr/local/bin/rollup --config /opt/rollup/rollup.toml
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

## Monitoring & Health Checks

### Health Endpoints
```bash
# System health
curl http://localhost:8080/health

# Component health  
curl http://localhost:8080/health/reth
curl http://localhost:8080/health/exex
```

### Key Metrics
```bash
# Prometheus metrics at localhost:9090/metrics
rollup_exex_notifications_processed_total
rollup_buffer_size_current
rollup_exex_lag_blocks
rollup_reorgs_detected_total
rollup_service_uptime_seconds
```

### RPC Testing
```bash
# Test HTTP RPC
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://localhost:8545

# Test Engine API (with JWT)
curl -X POST -H "Authorization: Bearer $JWT_TOKEN" \
  --data '{"jsonrpc":"2.0","method":"engine_exchangeCapabilities","params":[[]],"id":1}' \
  http://localhost:8551
```

## Troubleshooting

### Common Issues

**Port conflicts:**
```bash
netstat -tlnp | grep :8545
# Change ports or stop conflicting services
```

**High memory usage:**
```bash
# Reduce buffer size
--kona-buffer-size 500

# Monitor memory in metrics
curl localhost:9090/metrics | grep memory
```

**Slow sync:**
```bash
# Increase concurrency
--kona-max-concurrent-requests 15

# Check processing lag
curl localhost:9090/metrics | grep rollup_exex_lag
```

**Configuration errors:**
```bash
# Validate config
./rollup validate --config rollup.toml

# Test connectivity
./rollup validate --config rollup.toml --check-connectivity
```

### Debug Mode
```bash
# Enable debug logging
RUST_LOG=debug ./rollup --config rollup.toml

# Component-specific debugging
RUST_LOG=kona_rollup=debug,reth=info ./rollup --config rollup.toml
```

## Performance Tuning

### Memory Optimization
- `kona.buffer_size`: 1000 (default), 2000+ for high-throughput
- `reth.database.cache_size`: 2048MB+ for better performance

### Network Optimization  
- `reth.max_peers`: 50-100 for better connectivity
- Use WebSocket URLs for real-time L1/L2 updates

### System Requirements
- **Minimum**: 8GB RAM, 2 cores, 500GB SSD
- **Recommended**: 16GB RAM, 4+ cores, 1TB NVMe

## Security Best Practices

### Production Deployment
```bash
# Use environment variables for secrets
export ROLLUP_L1_RPC_URL="https://..."

# Bind RPC to localhost only
--http-addr 127.0.0.1

# Generate secure JWT secret
export JWT_SECRET="0x$(openssl rand -hex 32)"

# Run as non-root user
sudo -u rollup ./rollup --config config.toml
```

### Firewall Rules
```bash
# Only expose necessary ports
ufw allow 30303  # P2P
ufw allow 9090   # Metrics (internal only)
# Don't expose 8545/8551 externally
```

## Support

- **Documentation**: See config examples in `docs/config-examples/`
- **Issues**: [GitHub Issues](https://github.com/op-rs/kona/issues) 
- **Discussions**: [GitHub Discussions](https://github.com/op-rs/kona/discussions)