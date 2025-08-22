//! NodeBuilder API integration for running op-reth with KonaNodeExEx.

use anyhow::Result;
use kona_cli::NodeCliConfig;
use tracing::info;

/// Runs op-reth node with KonaNodeExEx installed using the NodeBuilder API framework.
///
/// This function demonstrates the proper configuration mapping for reth's CLI
/// infrastructure. The actual NodeBuilder integration faces complex trait bound
/// challenges that require further resolution in reth's type system.
pub async fn run_op_reth_with_kona_exex(kona_config: NodeCliConfig) -> Result<()> {
    info!("Starting op-reth node with KonaNodeExEx using NodeBuilder API");

    // Construct op-reth CLI arguments based on kona configuration
    let cli_args = construct_op_reth_args(&kona_config)?;
    
    info!("Constructed op-reth CLI args: {:?}", cli_args);

    // NOTE: Direct reth CLI integration faces complex trait bound challenges.
    // The reth NodeBuilder API requires very specific type configurations that are
    // difficult to satisfy manually. The CLI approach that would work requires
    // synchronous execution and additional complexity.
    //
    // For now, we demonstrate the configuration mapping and provide a foundation
    // for future integration once these trait bounds are resolved.
    
    info!("Kona configuration successfully mapped to op-reth arguments:");
    for arg in &cli_args {
        info!("  {}", arg);
    }
    
    // TODO: Complete reth CLI integration
    // The following would be the approach once trait bounds are resolved:
    // let cli = Cli::try_parse_args_from(cli_args)?;
    // cli.run(|builder, _args| {
    //     builder
    //         .node(OpNode::default())
    //         .install_exex("KonaNode", |ctx| {
    //             let exex = KonaNodeExEx::new_with_config(ctx, kona_config)?;
    //             exex.start()
    //         })
    //         .launch()
    // })
    
    info!("✓ NodeBuilder integration framework ready");
    info!("Note: Full reth CLI integration pending trait bound resolution");
    
    // Keep the process running for demonstration
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");
    
    Ok(())
}

/// Constructs op-reth CLI arguments from Kona configuration.
fn construct_op_reth_args(config: &NodeCliConfig) -> Result<Vec<String>> {
    let mut args = vec![
        "op-reth".to_string(),
        "node".to_string(),
        "--chain".to_string(),
        "optimism".to_string(),
    ];

    // Add L1 RPC configuration
    args.extend_from_slice(&[
        "--rollup.sequencer-http".to_string(),
        config.l1_eth_rpc.to_string(),
    ]);

    // Add L1 beacon configuration if available
    args.extend_from_slice(&[
        "--rollup.l1-beacon-url".to_string(),
        config.l1_beacon.to_string(),
    ]);

    // Add data directory
    args.extend_from_slice(&[
        "--datadir".to_string(),
        "./datadir".to_string(),
    ]);

    // Add P2P configuration if not disabled
    if !config.p2p.no_discovery {
        args.extend_from_slice(&[
            "--port".to_string(),
            config.p2p.listen_tcp_port.to_string(),
        ]);
        
        args.extend_from_slice(&[
            "--addr".to_string(),
            config.p2p.listen_ip.to_string(),
        ]);
    } else {
        args.push("--disable-discovery".to_string());
    }

    // Add RPC configuration if not disabled
    if !config.rpc.disabled {
        args.push("--http".to_string());
        args.extend_from_slice(&[
            "--http.addr".to_string(),
            config.rpc.listen_addr.to_string(),
            "--http.port".to_string(),
            config.rpc.listen_port.to_string(),
        ]);

        if config.rpc.ws_enabled {
            args.push("--ws".to_string());
        }

        if config.rpc.enable_admin {
            args.push("--http.api".to_string());
            args.push("admin,debug,eth,net,trace,txpool,web3".to_string());
        }
    }

    // Add sequencer configuration for sequencer mode
    if matches!(config.mode, kona_cli::node_config::NodeMode::Sequencer) {
        if let Some(conductor_rpc) = &config.sequencer.conductor_rpc {
            args.extend_from_slice(&[
                "--rollup.conductor-rpc".to_string(),
                conductor_rpc.to_string(),
            ]);
        }
        
        if config.sequencer.stopped {
            args.push("--rollup.sequencer-stopped".to_string());
        }
        
        args.extend_from_slice(&[
            "--rollup.sequencer-max-safe-lag".to_string(),
            config.sequencer.max_safe_lag.to_string(),
        ]);
    }

    info!("Mapped Kona config to op-reth CLI arguments");
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kona_cli::node_config::{NodeMode, P2PConfig, RpcConfig, SequencerConfig};
    use std::net::{IpAddr, Ipv4Addr};
    use url::Url;

    fn default_test_config() -> NodeCliConfig {
        NodeCliConfig {
            mode: NodeMode::Validator,
            l1_eth_rpc: Url::parse("http://localhost:8545").unwrap(),
            l1_trust_rpc: true,
            l1_beacon: Url::parse("http://localhost:5052").unwrap(),
            l2_trust_rpc: true,
            l2_config_file: None,
            global: kona_cli::node_config::GlobalConfig {
                l2_chain_id: 10,
                fork_overrides: Default::default(),
            },
            p2p: P2PConfig {
                no_discovery: false,
                priv_path: None,
                listen_ip: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
                listen_tcp_port: 9222,
                listen_udp_port: 9223,
                peers_lo: 30,
                peers_hi: 60,
                scoring: "light".to_string(),
                ban_enabled: false,
                unsafe_block_signer: None,
            },
            rpc: RpcConfig {
                disabled: false,
                listen_addr: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                listen_port: 8551,
                enable_admin: false,
                admin_persistence: None,
                ws_enabled: false,
                dev_enabled: false,
            },
            sequencer: SequencerConfig {
                stopped: false,
                max_safe_lag: 1024,
                l1_confs: 4,
                recover: false,
                conductor_rpc: None,
            },
        }
    }

    #[test]
    fn test_construct_basic_args() {
        let config = default_test_config();
        let args = construct_op_reth_args(&config).unwrap();
        
        assert!(args.contains(&"op-reth".to_string()));
        assert!(args.contains(&"node".to_string()));
        assert!(args.contains(&"--chain".to_string()));
        assert!(args.contains(&"optimism".to_string()));
        assert!(args.contains(&"--rollup.sequencer-http".to_string()));
        assert!(args.contains(&"http://localhost:8545/".to_string()));
    }

    #[test]
    fn test_construct_sequencer_mode_args() {
        let mut config = default_test_config();
        config.mode = NodeMode::Sequencer;
        config.sequencer.conductor_rpc = Some(Url::parse("http://localhost:8555").unwrap());
        
        let args = construct_op_reth_args(&config).unwrap();
        
        assert!(args.contains(&"--rollup.conductor-rpc".to_string()));
        assert!(args.contains(&"http://localhost:8555/".to_string()));
    }

    #[test]
    fn test_construct_p2p_disabled_args() {
        let mut config = default_test_config();
        config.p2p.no_discovery = true;
        
        let args = construct_op_reth_args(&config).unwrap();
        
        assert!(args.contains(&"--disable-discovery".to_string()));
        assert!(!args.contains(&"--port".to_string()));
    }

    #[test]
    fn test_construct_rpc_disabled_args() {
        let mut config = default_test_config();
        config.rpc.disabled = true;
        
        let args = construct_op_reth_args(&config).unwrap();
        
        assert!(!args.contains(&"--http".to_string()));
        assert!(!args.contains(&"--http.addr".to_string()));
    }

    #[test]
    fn test_construct_with_websocket_args() {
        let mut config = default_test_config();
        config.rpc.ws_enabled = true;
        
        let args = construct_op_reth_args(&config).unwrap();
        
        assert!(args.contains(&"--ws".to_string()));
    }

    #[test]
    fn test_construct_with_admin_api_args() {
        let mut config = default_test_config();
        config.rpc.enable_admin = true;
        
        let args = construct_op_reth_args(&config).unwrap();
        
        assert!(args.contains(&"--http.api".to_string()));
        assert!(args.contains(&"admin,debug,eth,net,trace,txpool,web3".to_string()));
    }
}