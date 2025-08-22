//! Configuration types for the rollup service.
//!
//! This module provides configuration structs for service orchestration and ExEx integration,
//! designed to be simple and focused on the service layer concerns.

use crate::error::{RollupError, RollupResult, ServiceError};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use url::Url;

/// Configuration for the rollup service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollupConfig {
    /// Global configuration
    pub global: GlobalConfig,

    /// Service orchestration configuration
    pub service: ServiceConfig,

    /// ExEx integration configuration
    pub exex: ExExConfig,
}

/// Global configuration shared across components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Chain identifier (e.g., "10" for Optimism, "8453" for Base)
    pub chain: String,

    /// Primary data directory
    pub datadir: PathBuf,

    /// Log level
    pub log_level: String,

    /// L1 RPC endpoint
    pub l1_rpc_url: Url,

    /// L2 Engine RPC endpoint (connects to local op-reth)
    pub l2_engine_url: Url,
}

/// Service orchestration configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Graceful shutdown timeout
    pub shutdown_timeout: Duration,

    /// Health check interval
    pub health_check_interval: Duration,

    /// Maximum startup time before considering service failed
    pub startup_timeout: Duration,

    /// Enable automatic restart on recoverable errors
    pub auto_restart: bool,

    /// Maximum restart attempts before giving up
    pub max_restart_attempts: u32,

    /// Delay between restart attempts
    pub restart_delay: Duration,
}

/// ExEx integration configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExExConfig {
    /// Notification processing timeout in seconds
    pub notification_timeout: u64,

    /// Maximum concurrent notification processing tasks
    pub max_concurrent_notifications: usize,

    /// Buffer size for pending notifications
    pub notification_buffer_size: usize,

    /// Batch size for block processing
    pub batch_size: usize,

    /// Enable debug logging for ExEx
    pub debug: bool,

    /// Backpressure threshold
    pub backpressure_threshold: usize,

    /// Processing queue depth
    pub processing_queue_depth: usize,
}

impl Default for RollupConfig {
    fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            service: ServiceConfig::default(),
            exex: ExExConfig::default(),
        }
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            chain: "10".to_string(), // Default to Optimism mainnet
            datadir: dirs::home_dir()
                .map(|home| home.join(".kona-rollup"))
                .unwrap_or_else(|| PathBuf::from("./data")),
            log_level: "info".to_string(),
            l1_rpc_url: "http://localhost:8545".parse().expect("valid URL"),
            l2_engine_url: "http://localhost:8551".parse().expect("valid URL"),
        }
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(10),
            startup_timeout: Duration::from_secs(60),
            auto_restart: true,
            max_restart_attempts: 3,
            restart_delay: Duration::from_secs(5),
        }
    }
}

impl Default for ExExConfig {
    fn default() -> Self {
        Self {
            notification_timeout: 30,
            max_concurrent_notifications: 5,
            notification_buffer_size: 100,
            batch_size: 100,
            debug: false,
            backpressure_threshold: 80,
            processing_queue_depth: 1000,
        }
    }
}

impl RollupConfig {
    /// Validate the configuration for consistency and required fields.
    pub fn validate(&self) -> RollupResult<()> {
        // Validate chain configuration
        if self.global.chain.is_empty() {
            return Err(RollupError::Service(ServiceError::InitializationFailed {
                component: "config".to_string(),
                message: "Chain ID cannot be empty".to_string(),
            }));
        }

        // Validate service configuration
        if self.exex.backpressure_threshold == 0 || self.exex.backpressure_threshold > 100 {
            return Err(RollupError::Service(ServiceError::InitializationFailed {
                component: "config".to_string(),
                message: "Backpressure threshold must be between 1 and 100".to_string(),
            }));
        }

        if self.exex.notification_buffer_size == 0 {
            return Err(RollupError::Service(ServiceError::InitializationFailed {
                component: "config".to_string(),
                message: "Notification buffer size must be greater than 0".to_string(),
            }));
        }

        // Ensure data directory is creatable/writable
        if let Err(e) = std::fs::create_dir_all(&self.global.datadir) {
            return Err(RollupError::Service(ServiceError::InitializationFailed {
                component: "config".to_string(),
                message: format!(
                    "Cannot create data directory {}: {}",
                    self.global.datadir.display(),
                    e
                ),
            }));
        }

        Ok(())
    }

    /// Get the effective L1 RPC URL.
    pub fn l1_rpc_url(&self) -> &Url {
        &self.global.l1_rpc_url
    }

    /// Get the effective L2 engine URL.
    pub fn l2_engine_url(&self) -> &Url {
        &self.global.l2_engine_url
    }

    /// Load configuration from a JSON file.
    pub fn from_file(path: impl Into<PathBuf>) -> RollupResult<Self> {
        let path = path.into();
        let content = std::fs::read_to_string(&path).map_err(|e| {
            RollupError::Service(ServiceError::InitializationFailed {
                component: "config".to_string(),
                message: format!("Failed to read config file {}: {}", path.display(), e),
            })
        })?;

        let config: Self = serde_json::from_str(&content).map_err(|e| {
            RollupError::Service(ServiceError::InitializationFailed {
                component: "config".to_string(),
                message: format!("Failed to parse config file {}: {}", path.display(), e),
            })
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to a JSON file.
    pub fn save_to_file(&self, path: impl Into<PathBuf>) -> RollupResult<()> {
        let path = path.into();
        let content = serde_json::to_string_pretty(self).map_err(|e| {
            RollupError::Service(ServiceError::RuntimeError {
                component: "config".to_string(),
                message: format!("Failed to serialize configuration: {}", e),
            })
        })?;

        std::fs::write(&path, content).map_err(|e| {
            RollupError::Service(ServiceError::RuntimeError {
                component: "config".to_string(),
                message: format!("Failed to write config file {}: {}", path.display(), e),
            })
        })?;

        Ok(())
    }

    /// Merge environment variables into the configuration.
    pub fn merge_env_vars(&mut self) -> RollupResult<()> {
        // L1 RPC URL
        if let Ok(l1_rpc) = std::env::var("ROLLUP_L1_RPC_URL") {
            self.global.l1_rpc_url = l1_rpc.parse().map_err(|e| {
                RollupError::Service(ServiceError::InitializationFailed {
                    component: "config".to_string(),
                    message: format!("Invalid L1 RPC URL in environment: {}", e),
                })
            })?;
        }

        // L2 Engine URL
        if let Ok(l2_engine) = std::env::var("ROLLUP_L2_ENGINE_URL") {
            self.global.l2_engine_url = l2_engine.parse().map_err(|e| {
                RollupError::Service(ServiceError::InitializationFailed {
                    component: "config".to_string(),
                    message: format!("Invalid L2 engine URL in environment: {}", e),
                })
            })?;
        }

        // Chain ID
        if let Ok(chain) = std::env::var("ROLLUP_CHAIN") {
            self.global.chain = chain;
        }

        // Data directory
        if let Ok(datadir) = std::env::var("ROLLUP_DATADIR") {
            self.global.datadir = PathBuf::from(datadir);
        }

        // Log level
        if let Ok(log_level) = std::env::var("ROLLUP_LOG_LEVEL") {
            self.global.log_level = log_level;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = RollupConfig::default();

        assert_eq!(config.global.chain, "10");
        assert!(config.global.datadir.to_string_lossy().contains(".kona-rollup"));
        assert_eq!(config.global.log_level, "info");
        assert_eq!(config.service.shutdown_timeout, Duration::from_secs(30));
        assert_eq!(config.exex.notification_timeout, 30);
    }

    #[test]
    fn test_config_validation() {
        let mut config = RollupConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid chain should fail
        config.global.chain = String::new();
        assert!(config.validate().is_err());

        // Reset and test invalid backpressure threshold
        config.global.chain = "10".to_string();
        config.exex.backpressure_threshold = 0;
        assert!(config.validate().is_err());

        config.exex.backpressure_threshold = 101;
        assert!(config.validate().is_err());

        // Reset and test invalid buffer size
        config.exex.backpressure_threshold = 80;
        config.exex.notification_buffer_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = RollupConfig::default();

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: RollupConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(config.global.chain, deserialized.global.chain);
        assert_eq!(config.global.l1_rpc_url, deserialized.global.l1_rpc_url);
    }

    #[test]
    fn test_env_var_merging() {
        let mut config = RollupConfig::default();

        // Set environment variables
        unsafe {
            env::set_var("ROLLUP_L1_RPC_URL", "http://example.com:8545");
            env::set_var("ROLLUP_CHAIN", "8453");
            env::set_var("ROLLUP_LOG_LEVEL", "debug");
        }

        config.merge_env_vars().unwrap();

        assert_eq!(config.global.l1_rpc_url.as_str(), "http://example.com:8545/");
        assert_eq!(config.global.chain, "8453");
        assert_eq!(config.global.log_level, "debug");

        // Clean up
        unsafe {
            env::remove_var("ROLLUP_L1_RPC_URL");
            env::remove_var("ROLLUP_CHAIN");
            env::remove_var("ROLLUP_LOG_LEVEL");
        }
    }
}
