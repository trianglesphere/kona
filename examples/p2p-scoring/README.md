# Kona P2P Scoring Configuration Example

This example demonstrates comprehensive peer and topic scoring configuration for kona-p2p networks. It provides practical patterns for implementing scoring in different network scenarios, from testing environments to production deployments.

## Overview

The example showcases:

- **Scoring Profiles**: Predefined configurations for different network scenarios
- **Topic Scoring**: Advanced mesh optimization through topic-specific parameters
- **Peer Monitoring**: Real-time tracking of peer behavior and network health
- **Configuration Management**: Flexible parameter tuning and validation

## Quick Start

```bash
# Run with default conservative scoring
cargo run --release -p example-p2p-scoring

# Try different scoring profiles
cargo run --release -p example-p2p-scoring -- --scoring-profile moderate
cargo run --release -p example-p2p-scoring -- --scoring-profile aggressive

# Enable topic scoring for enhanced performance
cargo run --release -p example-p2p-scoring -- --topic-scoring

# View detailed configuration
cargo run --release -p example-p2p-scoring -- --show-config
```

## Scoring Profiles

### Off Profile
- **Use Case**: Testing, debugging, permissioned networks
- **Security**: No protection against malicious peers
- **Performance**: Higher bandwidth usage, no scoring overhead
- **Ban Threshold**: N/A (no banning)

### Conservative Profile (Default)
- **Use Case**: New networks, gradual scoring rollout
- **Security**: Basic protection with lenient thresholds
- **Performance**: Minimal scoring overhead
- **Ban Threshold**: -20.0 (5-minute bans)

### Moderate Profile
- **Use Case**: Production networks with good peer diversity
- **Security**: Balanced protection for most scenarios
- **Performance**: Optimal for general-purpose deployments
- **Ban Threshold**: -40.0 (15-minute bans)

### Aggressive Profile
- **Use Case**: High-security or performance-critical networks
- **Security**: Strong protection against malicious behavior
- **Performance**: Higher overhead but better peer filtering
- **Ban Threshold**: -60.0 (30-minute bans)

## Topic Scoring

Topic scoring enhances mesh performance by tracking peer behavior on specific topics:

```bash
# Enable topic scoring
cargo run --release -p example-p2p-scoring -- --topic-scoring
```

**Key Parameters**:
- **Time in Mesh Weight**: Rewards peers for mesh participation
- **First Message Deliveries**: Tracks unique message delivery
- **Mesh Message Deliveries**: Monitors mesh reliability
- **Invalid Message Penalties**: Punishes bad behavior

## Command Line Options

```
Options:
  -c, --l2-chain-id <L2_CHAIN_ID>
          The L2 chain ID to use [default: 10]

  -g, --gossip-port <GOSSIP_PORT>
          Port to listen for gossip on [default: 9099]

  -d, --disc-port <DISC_PORT>
          Port to listen for discovery on [default: 9098]

  -i, --discovery-interval <DISCOVERY_INTERVAL>
          Interval to send discovery packets in seconds [default: 5]

  -s, --scoring-profile <SCORING_PROFILE>
          Scoring profile: off, conservative, moderate, aggressive [default: conservative]

      --topic-scoring
          Enable topic scoring for enhanced mesh performance

      --ban-threshold <BAN_THRESHOLD>
          Threshold below which peers are banned (negative values)

      --ban-duration <BAN_DURATION>
          Duration for which banned peers remain banned

      --monitoring
          Enable network monitoring and metrics collection [default: true]

      --show-config
          Print detailed scoring configuration and exit

  -v, --verbosity <VERBOSITY>
          Verbosity level [default: 0]

  -h, --help
          Print help (see a summary with '-h')
```

## Configuration Examples

### Development/Testing
```bash
# No scoring for local testing
cargo run -p example-p2p-scoring -- --scoring-profile off --verbosity 2
```

### Production Deployment
```bash
# Moderate scoring with topic optimization
cargo run -p example-p2p-scoring -- \
  --scoring-profile moderate \
  --topic-scoring \
  --discovery-interval 10
```

### High-Security Network
```bash
# Aggressive scoring with custom ban parameters
cargo run -p example-p2p-scoring -- \
  --scoring-profile aggressive \
  --topic-scoring \
  --ban-threshold -80.0 \
  --ban-duration 3600
```

## Monitoring and Metrics

The example includes comprehensive monitoring:

- **Block Reception**: Track block arrival and processing
- **Peer Events**: Monitor connections, disconnections, and bans
- **Message Statistics**: Analyze message validity and propagation
- **Network Health**: Overall network performance scoring

**Monitoring Output Example**:
```
[INFO] Network Statistics (uptime: 5m30s)
[INFO] Blocks: 45 total, 8 recent (rate: 1.45/min)
[INFO] Peers: 12 current, 15 total connections, 3 disconnections, 1 banned
[INFO] Messages: 234 total, 96.2% valid, 2.1% duplicates
```

## Architecture

### Key Components

1. **ScoringConfig**: Central configuration management
2. **ScoringProfile**: Predefined parameter sets
3. **NetworkMonitor**: Real-time metrics and health tracking
4. **Integration**: Seamless kona-p2p integration

### Configuration Structure

```rust
pub struct ScoringConfig {
    pub profile: ScoringProfile,
    pub topic_scoring: bool,
    pub ban_threshold: f64,
    pub ban_duration: Duration,
    pub mesh_config: MeshConfig,
    pub timing_config: TimingConfig,
}
```

## Security Considerations

### Scoring Profile Selection
- **Off**: Only for trusted/testing environments
- **Conservative**: Good for network establishment phase
- **Moderate**: Recommended for most production use
- **Aggressive**: For high-security or attack-prone networks

### Parameter Tuning
- Lower ban thresholds increase security but may cause isolation
- Shorter ban durations allow faster peer recovery
- Topic scoring improves quality but increases computational overhead

### Monitoring Requirements
- Regular monitoring prevents configuration drift
- Health scores help identify network issues early
- Ban event analysis reveals attack patterns

## Performance Tuning

### Mesh Size Configuration
```rust
// Conservative: smaller mesh, lower overhead
mesh_n: 8, mesh_n_low: 6, mesh_n_high: 12

// Aggressive: larger mesh, better redundancy  
mesh_n: 12, mesh_n_low: 10, mesh_n_high: 18
```

### Timing Parameters
```rust
// Standard timing for most networks
heartbeat_interval: Duration::from_millis(500),
fanout_ttl: Duration::from_secs(60),
```

## Integration with Kona

This example integrates with the broader kona ecosystem:

- **kona-p2p**: Core P2P functionality
- **kona-peers**: Peer management and scoring
- **kona-node-service**: Network service orchestration
- **kona-registry**: Chain configuration management

## Troubleshooting

### Common Issues

1. **No Peers Connecting**
   - Try `--scoring-profile off` for debugging
   - Check firewall and network connectivity
   - Verify bootstrap nodes are reachable

2. **Excessive Peer Bans**
   - Use more lenient profile (`conservative`)
   - Increase ban threshold (less negative)
   - Disable topic scoring temporarily

3. **Poor Network Performance**
   - Enable topic scoring for better mesh
   - Increase mesh size parameters
   - Monitor for network partitioning

### Debug Commands

```bash
# Maximum verbosity for debugging
cargo run -p example-p2p-scoring -- --verbosity 2

# Show detailed configuration
cargo run -p example-p2p-scoring -- --show-config

# Disable scoring for connectivity testing
cargo run -p example-p2p-scoring -- --scoring-profile off
```

## Contributing

When extending this example:

1. Follow kona coding standards (MSRV 1.82, no warnings)
2. Add comprehensive tests for new features
3. Update documentation and examples
4. Consider security implications of parameter changes

## License

This example is part of the kona project and follows the same licensing terms.