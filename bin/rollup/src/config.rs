//! Configuration management for the rollup binary.

use crate::{
    cli::{GlobalArgs, NodeCommand},
    error::{RollupError, RollupResult},
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf};
use url::Url;

/// Unified configuration for the rollup binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollupConfig {
    /// Global configuration.
    pub global: GlobalConfig,
    
    /// Node configuration.
    pub node: NodeConfig,
}

/// Global configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Chain identifier.
    pub chain: String,
    
    /// Primary data directory.
    pub datadir: PathBuf,
    
    /// Log level.
    pub log_level: String,
    
    /// Enable colored logging output.
    pub log_color: bool,
    
    /// Configuration file path (optional).
    pub config_file: Option<PathBuf>,
}

/// Node configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// L1 RPC endpoint.
    pub l1_rpc_url: Url,
    
    /// L2 RPC listen address.
    pub l2_rpc_addr: SocketAddr,
    
    /// Engine API listen address.
    pub engine_addr: SocketAddr,
    
    /// P2P listen address.
    pub p2p_addr: SocketAddr,
    
    /// JWT secret path.
    pub jwt_secret: Option<PathBuf>,
    
    /// Enable metrics.
    pub metrics: bool,
    
    /// Metrics listen address.
    pub metrics_addr: SocketAddr,
}

impl RollupConfig {
    /// Create configuration from CLI arguments.
    pub fn from_cli(global: GlobalArgs, node: NodeCommand) -> RollupResult<Self> {
        let datadir = global.datadir.unwrap_or_else(|| {
            dirs::home_dir()
                .map(|home| home.join(".kona-rollup"))
                .unwrap_or_else(|| PathBuf::from("./data"))
        });

        let config = Self {
            global: GlobalConfig {
                chain: global.chain,
                datadir: datadir.clone(),
                log_level: "info".to_string(),
                log_color: !global.no_color,
                config_file: global.config,
            },
            node: NodeConfig {
                l1_rpc_url: node.l1_rpc_url,
                l2_rpc_addr: node.l2_rpc_addr,
                engine_addr: node.engine_addr,
                p2p_addr: node.p2p_addr,
                jwt_secret: node.jwt_secret,
                metrics: node.metrics,
                metrics_addr: node.metrics_addr,
            },
        };

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a file.
    pub fn from_file(path: &PathBuf) -> RollupResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)
            .map_err(|e| RollupError::Config(format!("Failed to parse config file {}: {}", path.display(), e)))?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to a file.
    pub fn save_to_file(&self, path: &PathBuf) -> RollupResult<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| RollupError::Config(format!("Failed to serialize configuration: {}", e)))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate the configuration.
    pub fn validate(&self) -> RollupResult<()> {
        // Validate chain configuration
        if self.global.chain.is_empty() {
            return Err(RollupError::Config("Chain ID cannot be empty".to_string()));
        }

        // Validate JWT secret exists if provided
        if let Some(jwt_path) = &self.node.jwt_secret {
            if !jwt_path.exists() {
                return Err(RollupError::Config(format!(
                    "JWT secret file not found: {}", 
                    jwt_path.display()
                )));
            }
        }

        // Ensure data directory is creatable/writable
        if let Err(e) = std::fs::create_dir_all(&self.global.datadir) {
            return Err(RollupError::Config(format!(
                "Cannot create data directory {}: {}", 
                self.global.datadir.display(), 
                e
            )));
        }

        Ok(())
    }

    /// Merge environment variables into the configuration.
    pub fn merge_env_vars(&mut self) -> RollupResult<()> {
        // L1 RPC URL
        if let Ok(l1_rpc) = std::env::var("ROLLUP_L1_RPC_URL") {
            self.node.l1_rpc_url = l1_rpc.parse()?;
        }

        // JWT Secret
        if let Ok(jwt_path) = std::env::var("ROLLUP_JWT_SECRET") {
            self.node.jwt_secret = Some(PathBuf::from(jwt_path));
        }

        // Chain ID
        if let Ok(chain) = std::env::var("ROLLUP_CHAIN") {
            self.global.chain = chain;
        }

        // Data directory
        if let Ok(datadir) = std::env::var("ROLLUP_DATADIR") {
            self.global.datadir = PathBuf::from(datadir);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::NodeCommand;

    fn create_test_node_command() -> NodeCommand {
        NodeCommand {
            l1_rpc_url: "http://localhost:8545".parse().unwrap(),
            l2_rpc_addr: "127.0.0.1:8545".parse().unwrap(),
            engine_addr: "127.0.0.1:8551".parse().unwrap(),
            p2p_addr: "0.0.0.0:30303".parse().unwrap(),
            jwt_secret: None,
            metrics: false,
            metrics_addr: "127.0.0.1:9090".parse().unwrap(),
        }
    }

    #[test]
    fn test_config_from_cli() {
        let global = GlobalArgs {
            verbose: 0,
            no_color: false,
            config: None,
            datadir: None,
            chain: "10".to_string(),
        };
        
        let node = create_test_node_command();
        let config = RollupConfig::from_cli(global, node).unwrap();
        
        assert_eq!(config.global.chain, "10");
        assert_eq!(config.node.l1_rpc_url.as_str(), "http://localhost:8545/");
    }

    #[test]
    fn test_config_serialization() {
        let global = GlobalArgs {
            verbose: 0,
            no_color: false,
            config: None,
            datadir: None,
            chain: "10".to_string(),
        };
        
        let node = create_test_node_command();
        let config = RollupConfig::from_cli(global, node).unwrap();
        
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: RollupConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.global.chain, deserialized.global.chain);
        assert_eq!(config.node.l1_rpc_url, deserialized.node.l1_rpc_url);
    }
}