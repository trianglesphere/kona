//! # Rollup Service
//!
//! This module implements the main service orchestration for the unified rollup binary,
//! managing the lifecycle of both the op-reth node and the kona-node ExEx integration
//! to provide a seamless, single-binary rollup solution.

use crate::{
    config::{ExExConfig, RollupConfig, ServiceConfig},
    error::{ExExError, RollupError, RollupResult, ServiceError},
    exex::KonaNodeExEx,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    signal,
    sync::{broadcast, mpsc},
    time::interval,
};
use tracing::{Level, debug, error, info, span, warn};

/// Service states representing the current operational status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceState {
    /// Service created but not started
    Created,
    /// Service is initializing components
    Initializing,
    /// Service is starting components
    Starting,
    /// Service is running normally
    Running,
    /// Service is running but some components are degraded
    Degraded,
    /// Service is attempting to recover from failures
    Recovering,
    /// Service is shutting down gracefully
    Stopping,
    /// Service has stopped
    Stopped,
    /// Service encountered a fatal error
    Failed,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self::Created
    }
}

/// Component health status information.
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Whether the component is healthy
    pub healthy: bool,
    /// Last health check timestamp
    pub last_check: Instant,
    /// Optional error message if unhealthy
    pub error: Option<String>,
}

/// Service metrics and status information.
#[derive(Debug, Clone, Default)]
pub struct ServiceMetrics {
    /// Current service state
    pub state: ServiceState,
    /// Service start time
    pub start_time: Option<Instant>,
    /// Number of service restarts
    pub restart_count: u64,
    /// Component health status
    pub component_health: Vec<ComponentHealth>,
    /// Last health check time
    pub last_health_check: Option<Instant>,
}

/// Main rollup service orchestrating all components.
///
/// This service manages the complete lifecycle of the unified rollup binary,
/// coordinating the op-reth node and kona-node ExEx integration.
pub struct RollupService {
    /// Service configuration
    config: RollupConfig,

    /// Current service state
    state: Arc<Mutex<ServiceState>>,

    /// Service metrics
    metrics: Arc<Mutex<ServiceMetrics>>,

    /// Op-reth node handle (when running)
    reth_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Kona ExEx instance
    exex: Arc<Mutex<Option<KonaNodeExEx>>>,

    /// Shutdown signal broadcaster
    shutdown_tx: Arc<Mutex<Option<broadcast::Sender<()>>>>,

    /// Health check interval timer
    health_check_interval: Duration,

    /// Graceful shutdown timeout
    shutdown_timeout: Duration,
}

impl RollupService {
    /// Create a new rollup service instance.
    ///
    /// # Arguments
    /// * `config` - Service configuration
    ///
    /// # Returns
    /// * `Result<RollupService>` - New service instance or error
    ///
    /// # Example
    /// ```rust,no_run
    /// use rollup_service::{RollupConfig, RollupService};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = RollupConfig::default();
    ///     let service = RollupService::new(config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(config: RollupConfig) -> RollupResult<Self> {
        info!("Creating new rollup service");

        // Validate configuration
        config.validate()?;

        // Initialize service state
        let state = Arc::new(Mutex::new(ServiceState::Created));
        let metrics = Arc::new(Mutex::new(ServiceMetrics::default()));

        // Create shutdown broadcast channel
        let (shutdown_tx, _) = broadcast::channel(16);
        let shutdown_tx = Arc::new(Mutex::new(Some(shutdown_tx)));

        // Initialize handles
        let reth_handle = Arc::new(Mutex::new(None));
        let exex = Arc::new(Mutex::new(None));

        // Configure intervals
        let health_check_interval = config.service.health_check_interval;
        let shutdown_timeout = config.service.shutdown_timeout;

        info!("Rollup service created successfully");

        Ok(Self {
            config,
            state,
            metrics,
            reth_handle,
            exex,
            shutdown_tx,
            health_check_interval,
            shutdown_timeout,
        })
    }

    /// Start the rollup service and all components.
    ///
    /// This method initializes and starts all service components in the proper order,
    /// ensuring dependencies are satisfied before proceeding.
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn start(&mut self) -> RollupResult<()> {
        let _span = span!(Level::INFO, "service_start").entered();
        info!("Starting rollup service");

        // Update state to initializing
        {
            let mut state = self.state.lock().unwrap();
            *state = ServiceState::Initializing;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.state = ServiceState::Initializing;
            metrics.start_time = Some(Instant::now());
        }

        // Start components in order
        self.start_components().await?;

        // Start health monitoring
        self.start_health_monitoring().await?;

        // Start signal handling
        self.start_signal_handling().await?;

        // Update state to running
        {
            let mut state = self.state.lock().unwrap();
            *state = ServiceState::Running;
        }

        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.state = ServiceState::Running;
        }

        info!("Rollup service started successfully");
        Ok(())
    }

    /// Start all service components in proper dependency order.
    async fn start_components(&self) -> RollupResult<()> {
        info!("Starting service components");

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            *state = ServiceState::Starting;
        }

        // Start op-reth node first
        self.start_reth_node().await?;

        // Wait for reth node to be ready
        self.wait_for_reth_ready().await?;

        // Start kona ExEx
        self.start_kona_exex().await?;

        info!("All service components started");
        Ok(())
    }

    /// Start the op-reth node component.
    async fn start_reth_node(&self) -> RollupResult<()> {
        info!("Starting op-reth node");

        // TODO: Implement actual op-reth node building and execution
        // For now, we'll simulate with a placeholder
        let reth_handle = tokio::spawn(async move {
            info!("Op-reth node simulation started");

            // Simulate node startup
            tokio::time::sleep(Duration::from_secs(2)).await;

            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                // Simulate node work
            }
        });

        // Store the handle
        {
            let mut handle = self.reth_handle.lock().unwrap();
            *handle = Some(reth_handle);
        }

        info!("Op-reth node started");
        Ok(())
    }

    /// Wait for the reth node to be ready for ExEx installation.
    async fn wait_for_reth_ready(&self) -> RollupResult<()> {
        info!("Waiting for op-reth node readiness");

        let mut attempts = 0;
        let max_attempts = 30; // 30 seconds with 1-second intervals

        while attempts < max_attempts {
            if self.check_reth_ready().await {
                info!("Op-reth node is ready");
                return Ok(());
            }

            attempts += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Err(RollupError::Service(ServiceError::startup_failed(
            "reth",
            "Op-reth node failed to become ready within timeout",
        )))
    }

    /// Check if the reth node is ready.
    async fn check_reth_ready(&self) -> bool {
        // TODO: Implement actual readiness checks
        // For now, we'll simulate readiness after a delay
        static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let start = START_TIME.get_or_init(|| Instant::now());

        start.elapsed() > Duration::from_secs(3) // Simulate 3-second startup
    }

    /// Start the kona ExEx component.
    async fn start_kona_exex(&self) -> RollupResult<()> {
        info!("Starting kona ExEx");

        // Create ExEx instance
        let kona_exex = KonaNodeExEx::new(self.config.exex.clone()).await?;

        // Store the ExEx instance
        {
            let mut exex = self.exex.lock().unwrap();
            *exex = Some(kona_exex);
        }

        // TODO: Install the ExEx in the reth node when reth integration is available
        // For now, we'll just log success

        info!("Kona ExEx started successfully");
        Ok(())
    }

    /// Start health monitoring for all components.
    async fn start_health_monitoring(&self) -> RollupResult<()> {
        info!("Starting health monitoring");

        let state = Arc::clone(&self.state);
        let metrics = Arc::clone(&self.metrics);
        let reth_handle = Arc::clone(&self.reth_handle);
        let exex = Arc::clone(&self.exex);
        let interval_duration = self.health_check_interval;

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                interval.tick().await;

                // Check component health
                let mut component_health = Vec::new();

                // Check reth health
                let reth_healthy = {
                    let handle = reth_handle.lock().unwrap();
                    handle.as_ref().map_or(false, |h| !h.is_finished())
                };

                component_health.push(ComponentHealth {
                    name: "reth".to_string(),
                    healthy: reth_healthy,
                    last_check: Instant::now(),
                    error: if !reth_healthy {
                        Some("Component not running".to_string())
                    } else {
                        None
                    },
                });

                // Check ExEx health
                let exex_healthy = {
                    let exex_guard = exex.lock().unwrap();
                    if let Some(exex_instance) = exex_guard.as_ref() {
                        // TODO: Add actual health check when ExEx is fully implemented
                        true // Placeholder
                    } else {
                        false
                    }
                };

                component_health.push(ComponentHealth {
                    name: "exex".to_string(),
                    healthy: exex_healthy,
                    last_check: Instant::now(),
                    error: if !exex_healthy {
                        Some("Component not running".to_string())
                    } else {
                        None
                    },
                });

                // Update metrics
                {
                    let mut metrics = metrics.lock().unwrap();
                    metrics.component_health = component_health.clone();
                    metrics.last_health_check = Some(Instant::now());

                    // Update service state based on component health
                    let all_healthy = component_health.iter().all(|c| c.healthy);
                    let any_healthy = component_health.iter().any(|c| c.healthy);

                    let current_state = {
                        let state = state.lock().unwrap();
                        *state
                    };

                    let new_state = match current_state {
                        ServiceState::Running => {
                            if all_healthy {
                                ServiceState::Running
                            } else if any_healthy {
                                ServiceState::Degraded
                            } else {
                                ServiceState::Failed
                            }
                        }
                        ServiceState::Degraded => {
                            if all_healthy {
                                ServiceState::Running
                            } else if any_healthy {
                                ServiceState::Degraded
                            } else {
                                ServiceState::Failed
                            }
                        }
                        other => other, // Don't change state during startup/shutdown
                    };

                    if new_state != current_state {
                        let mut state = state.lock().unwrap();
                        *state = new_state;
                        metrics.state = new_state;

                        match new_state {
                            ServiceState::Degraded => warn!("Service entered degraded state"),
                            ServiceState::Failed => error!("Service failed - components unhealthy"),
                            ServiceState::Running => info!("Service recovered to running state"),
                            _ => {}
                        }
                    }
                }

                debug!("Health check completed: {} components checked", component_health.len());
            }
        });

        info!("Health monitoring started");
        Ok(())
    }

    /// Start signal handling for graceful shutdown.
    async fn start_signal_handling(&self) -> RollupResult<()> {
        info!("Starting signal handling");

        let shutdown_tx = Arc::clone(&self.shutdown_tx);

        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{SignalKind, signal};

                let mut sigterm =
                    signal(SignalKind::terminate()).expect("Failed to create SIGTERM handler");
                let mut sigint =
                    signal(SignalKind::interrupt()).expect("Failed to create SIGINT handler");

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM, initiating graceful shutdown");
                    }
                    _ = sigint.recv() => {
                        info!("Received SIGINT, initiating graceful shutdown");
                    }
                }
            }

            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
                info!("Received Ctrl+C, initiating graceful shutdown");
            }

            // Broadcast shutdown signal
            if let Some(tx) = shutdown_tx.lock().unwrap().as_ref() {
                let _ = tx.send(());
            }
        });

        info!("Signal handling started");
        Ok(())
    }

    /// Wait for shutdown signal and perform graceful shutdown.
    ///
    /// This method blocks until a shutdown signal is received, then coordinates
    /// the graceful shutdown of all service components.
    ///
    /// # Returns
    /// * `Result<()>` - Success or error during shutdown
    pub async fn wait_for_shutdown(&mut self) -> RollupResult<()> {
        info!("Waiting for shutdown signal");

        // Subscribe to shutdown signals
        let mut shutdown_rx = {
            let shutdown_tx = self.shutdown_tx.lock().unwrap();
            shutdown_tx.as_ref().unwrap().subscribe()
        };

        // Wait for shutdown signal
        let _ = shutdown_rx.recv().await;

        // Perform graceful shutdown
        self.shutdown().await
    }

    /// Perform graceful shutdown of all service components.
    ///
    /// # Returns
    /// * `Result<()>` - Success or error during shutdown
    pub async fn shutdown(&mut self) -> RollupResult<()> {
        let _span = span!(Level::INFO, "service_shutdown").entered();
        info!("Starting graceful shutdown");

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            *state = ServiceState::Stopping;
        }

        // Create shutdown timeout
        let shutdown_future = self.shutdown_components();

        match tokio::time::timeout(self.shutdown_timeout, shutdown_future).await {
            Ok(result) => result,
            Err(_) => {
                warn!("Shutdown timed out, forcing termination");
                self.force_shutdown().await
            }
        }
    }

    /// Shutdown all components gracefully.
    async fn shutdown_components(&self) -> RollupResult<()> {
        info!("Shutting down service components");

        // Stop ExEx first (reverse dependency order)
        {
            let mut exex = self.exex.lock().unwrap();
            if exex.take().is_some() {
                info!("Kona ExEx stopped");
            }
        }

        // Stop reth node
        if let Some(handle) = {
            let mut reth_handle = self.reth_handle.lock().unwrap();
            reth_handle.take()
        } {
            info!("Stopping op-reth node");
            handle.abort();
            let _ = handle.await;
            info!("Op-reth node stopped");
        }

        // Update final state
        {
            let mut state = self.state.lock().unwrap();
            *state = ServiceState::Stopped;
        }

        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.state = ServiceState::Stopped;
        }

        info!("All service components stopped");
        Ok(())
    }

    /// Force shutdown when graceful shutdown times out.
    async fn force_shutdown(&self) -> RollupResult<()> {
        warn!("Forcing immediate shutdown");

        // Abort all tasks
        {
            let mut exex = self.exex.lock().unwrap();
            exex.take();
        }

        if let Some(handle) = {
            let mut reth_handle = self.reth_handle.lock().unwrap();
            reth_handle.take()
        } {
            handle.abort();
            let _ = handle.await;
        }

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            *state = ServiceState::Stopped;
        }

        warn!("Force shutdown completed");
        Ok(())
    }

    /// Check if the service is healthy.
    ///
    /// # Returns
    /// * `bool` - True if the service and all components are healthy
    pub async fn is_healthy(&self) -> bool {
        let metrics = self.metrics.lock().unwrap();

        match metrics.state {
            ServiceState::Running => metrics.component_health.iter().all(|c| c.healthy),
            ServiceState::Degraded => metrics.component_health.iter().any(|c| c.healthy),
            _ => false,
        }
    }

    /// Get current service metrics.
    ///
    /// # Returns
    /// * `ServiceMetrics` - Current service metrics snapshot
    pub fn get_metrics(&self) -> ServiceMetrics {
        let metrics = self.metrics.lock().unwrap();
        metrics.clone()
    }

    /// Get current service state.
    ///
    /// # Returns
    /// * `ServiceState` - Current service state
    pub fn get_state(&self) -> ServiceState {
        let state = self.state.lock().unwrap();
        *state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_creation() {
        let config = RollupConfig::default();
        let service = RollupService::new(config).await.unwrap();

        assert_eq!(service.get_state(), ServiceState::Created);
        assert!(!service.is_healthy().await);
    }

    #[tokio::test]
    async fn test_service_state_transitions() {
        let mut config = RollupConfig::default();
        // Use a temporary directory for testing
        config.global.datadir = std::env::temp_dir().join("rollup-test");

        let mut service = RollupService::new(config).await.unwrap();

        assert_eq!(service.get_state(), ServiceState::Created);

        // Note: We can't fully test start() without mocking reth integration
        // This test validates the creation and basic state management
    }

    #[test]
    fn test_service_state_default() {
        let state = ServiceState::default();
        assert_eq!(state, ServiceState::Created);
    }

    #[test]
    fn test_service_metrics_default() {
        let metrics = ServiceMetrics::default();
        assert_eq!(metrics.state, ServiceState::Created);
        assert!(metrics.start_time.is_none());
        assert_eq!(metrics.restart_count, 0);
    }
}
