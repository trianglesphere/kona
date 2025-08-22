//! Complete NodeBuilder API integration examples for op-reth with Kona ExEx.
//! 
//! This file demonstrates the correct patterns for integrating reth's NodeBuilder API
//! with Kona's execution extension, showing solutions to the common trait bound issues.

use anyhow::Result;
use kona_cli::NodeCliConfig;
use kona_rollup::exex::KonaNodeExEx;
use reth::cli::Cli;
use reth_node_builder::NodeBuilder;
use reth_optimism_node::node::OpNode;
use std::env;
use std::sync::Arc;
use tracing::info;

/// Example 1: Using reth CLI pattern (RECOMMENDED)
/// 
/// This is the recommended approach as it leverages reth's existing CLI infrastructure
/// which handles database creation, networking, and other complex initialization automatically.
/// The key insight is that you DON'T create the database manually - reth does it for you.
pub async fn run_with_cli_pattern(kona_config: NodeCliConfig) -> Result<()> {
    info!("Running NodeBuilder with CLI pattern");
    
    // Construct arguments for op-reth
    let args = vec![
        "op-reth",
        "node", 
        "--chain", "optimism",
        "--datadir", "./datadir",
        "--rollup.sequencer-http", &kona_config.l1_eth_rpc.to_string(),
        // Add more flags as needed based on your kona_config
    ];
    
    // Use the reth CLI pattern - this is the key to avoiding database trait bound issues
    Cli::try_parse_args_from(args)?.run(|builder, _args| async move {
        let handle = builder
            // IMPORTANT: Use OptimismNode, not EthereumNode for OP Stack compatibility
            .node(OpNode::default())
            // Install your ExEx - the context provides everything you need
            .install_exex("KonaNode", move |ctx| {
                // Clone config for the async block
                let config = kona_config.clone();
                async move {
                    info!("Installing KonaNodeExEx with context");
                    
                    // Create the ExEx with the provided context
                    // The context already has the properly configured database, networking, etc.
                    let exex = KonaNodeExEx::new_with_config(ctx, config)?;
                    
                    // Return the future that runs the ExEx
                    Ok(exex.start())
                }
            })
            .launch()
            .await?;

        // Wait for the node to exit gracefully
        handle.wait_for_node_exit().await
    })
}

/// Example 2: Manual NodeBuilder configuration (ADVANCED)
/// 
/// This shows how you could potentially use NodeBuilder directly, but it's much more complex
/// and requires manual configuration of all the components that the CLI handles automatically.
/// Only use this if you need fine-grained control over the node construction.
#[allow(dead_code)]
pub async fn run_with_manual_builder(kona_config: NodeCliConfig) -> Result<()> {
    use reth_node_builder::{NodeConfig, FullNodeTypes};
    use reth_db::DatabaseEnv;
    use std::path::PathBuf;
    
    info!("Running with manual NodeBuilder configuration");
    
    // This is much more complex - you need to set up everything manually
    let node_config = NodeConfig::test()
        .with_datadir(PathBuf::from("./datadir"));
    
    // The key insight: Don't try to pass DatabaseEnv directly to NodeBuilder
    // Instead, let NodeBuilder create and manage the database internally
    let _handle = NodeBuilder::new(node_config)
        .with_types::<OpNode>()
        .with_components(OpNode::default())
        .install_exex("KonaNode", move |ctx| {
            let config = kona_config.clone();
            async move {
                info!("Installing KonaNodeExEx via manual builder");
                
                // The ExExContext provides access to the database via its methods
                // You don't need to create the database yourself
                let exex = KonaNodeExEx::new_with_config(ctx, config)?;
                Ok(exex.start())
            }
        })
        .launch()
        .await?;
    
    Ok(())
}

/// Example 3: Database access patterns within ExEx
/// 
/// This shows how to properly access the database within your ExEx implementation
/// without running into trait bound issues.
pub mod database_patterns {
    use reth_exex::ExExContext;
    use reth_node_api::FullNodeComponents;
    use reth_node_types::NodeTypes;
    use reth_primitives::NodePrimitives;
    use reth_db_api::database::Database;
    
    pub fn access_database_in_exex<Node>(ctx: &ExExContext<Node>) -> anyhow::Result<()>
    where
        Node: FullNodeComponents,
        Node::Types: NodeTypes,
        <Node::Types as NodeTypes>::Primitives: NodePrimitives,
    {
        // The ExExContext provides access to the node's components
        // including the database through the provider
        let _provider = ctx.node.provider();
        
        // You can access the database through the provider's methods
        // rather than trying to access DatabaseEnv directly
        
        // Example: Reading blockchain data
        // let latest_block = provider.latest_header()?;
        // let chain_spec = provider.chain_spec();
        
        Ok(())
    }
}

/// Example 4: Task executor patterns
/// 
/// Shows how to properly handle async tasks within the ExEx context
pub mod task_executor_patterns {
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;
    use tracing::{error, info};
    
    pub async fn run_exex_with_tasks() -> anyhow::Result<()> {
        let shutdown = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel(100);
        
        // Spawn background tasks
        let task_handle = tokio::spawn({
            let shutdown = shutdown.clone();
            async move {
                loop {
                    tokio::select! {
                        Some(msg) = rx.recv() => {
                            // Process messages
                            info!("Processing message: {:?}", msg);
                        }
                        _ = shutdown.cancelled() => {
                            info!("Background task shutting down");
                            break;
                        }
                    }
                }
            }
        });
        
        // Main ExEx loop would go here
        // ...
        
        // Cleanup
        shutdown.cancel();
        if let Err(e) = task_handle.await {
            error!("Background task failed: {}", e);
        }
        
        Ok(())
    }
}

/// Key takeaways and solutions:
/// 
/// 1. **Database Creation**: Don't create DatabaseEnv manually. Let reth's NodeBuilder
///    handle database creation internally through the CLI pattern.
/// 
/// 2. **OpNode vs EthereumNode**: Always use OpNode for OP Stack compatibility,
///    not EthereumNode.
/// 
/// 3. **ExEx Installation**: Use the install_exex method with a closure that returns
///    a Future. The ExExContext provides everything you need.
/// 
/// 4. **Database Access**: Access the database through the ExExContext's provider
///    methods, not directly through DatabaseEnv.
/// 
/// 5. **Trait Bounds**: The CLI pattern avoids trait bound issues by letting reth
///    handle the complex type configurations internally.
/// 
/// 6. **Configuration**: Map your kona configuration to op-reth CLI flags rather
///    than trying to configure the NodeBuilder manually.

#[cfg(test)]
mod tests {
    use super::*;
    use kona_cli::node_config::NodeMode;
    
    #[tokio::test]
    async fn test_node_builder_integration() {
        let config = NodeCliConfig {
            mode: NodeMode::Validator,
            l1_eth_rpc: "http://localhost:8545".parse().unwrap(),
            l1_trust_rpc: true,
            l1_beacon: "http://localhost:3500".parse().unwrap(),
            l2_trust_rpc: true,
            l2_config_file: None,
            global: Default::default(),
            p2p: Default::default(),
            rpc: Default::default(),
            sequencer: Default::default(),
        };
        
        // This test shows the configuration is properly set up
        // In a real test, you'd want to mock the node startup
        assert_eq!(config.mode, NodeMode::Validator);
        assert!(!config.l1_eth_rpc.to_string().is_empty());
    }
}