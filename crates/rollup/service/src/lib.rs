//! # Rollup Service
//!
//! This crate provides service orchestration and ExEx integration for the unified rollup binary.
//! It manages the lifecycle of both op-reth node and kona-node ExEx integration to provide
//! a seamless, single-binary rollup solution.
//!
//! ## Overview
//!
//! The rollup service acts as the top-level orchestrator that coordinates all components:
//!
//! - **Service Orchestration**: Manages component lifecycle and health monitoring
//! - **ExEx Integration**: Handles kona-node ExEx installation and coordination
//! - **Configuration Management**: Unified configuration for all components
//! - **Error Recovery**: Comprehensive error handling with recovery strategies
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use rollup_service::{RollupConfig, RollupService};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = RollupConfig::default();
//!     let mut service = RollupService::new(config).await?;
//!
//!     // Start the service
//!     service.start().await?;
//!
//!     // Wait for shutdown
//!     service.wait_for_shutdown().await?;
//!
//!     Ok(())
//! }
//! ```

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod config;
pub use config::{ExExConfig, RollupConfig, ServiceConfig};

mod error;
pub use error::{ExExError, RollupError, RollupResult, ServiceError};

mod service;
pub use service::{ComponentHealth, RollupService, ServiceMetrics, ServiceState};

mod exex;
pub use exex::{KonaNodeExEx, NotificationHandler};

/// Re-export commonly used types for convenience
pub mod prelude {
    pub use crate::{
        ComponentHealth, ExExConfig, ExExError, KonaNodeExEx, NotificationHandler, RollupConfig,
        RollupError, RollupResult, RollupService, ServiceConfig, ServiceError, ServiceMetrics,
        ServiceState,
    };
}
