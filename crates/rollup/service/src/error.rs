//! Error types for the rollup service.
//!
//! This module defines comprehensive error types with recovery strategies for all components
//! of the rollup service, including service orchestration and ExEx integration.

use thiserror::Error;

/// Main error type for rollup service operations.
#[derive(Error, Debug)]
pub enum RollupError {
    /// Service orchestration errors
    #[error("Service error: {0}")]
    Service(#[from] ServiceError),

    /// ExEx integration errors
    #[error("ExEx error: {0}")]
    ExEx(#[from] ExExError),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Other errors
    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Service orchestration and lifecycle management errors.
#[derive(Error, Debug)]
pub enum ServiceError {
    /// Service initialization failure
    #[error("Service initialization failed: {component} - {message}")]
    InitializationFailed { component: String, message: String },

    /// Service startup failure
    #[error("Service startup failed: {component} - {message}")]
    StartupFailed { component: String, message: String },

    /// Service runtime error
    #[error("Service runtime error: {component} - {message}")]
    RuntimeError { component: String, message: String },

    /// Health check failure
    #[error("Health check failed for {component}: {message}")]
    HealthCheckFailed { component: String, message: String },

    /// Service communication error
    #[error("Service communication error: {from} to {to} - {message}")]
    CommunicationError { from: String, to: String, message: String },

    /// Service dependency error
    #[error("Service dependency error: {service} depends on {dependency} - {message}")]
    DependencyError { service: String, dependency: String, message: String },

    /// Shutdown errors
    #[error("Shutdown error: {component} - {message}")]
    ShutdownError { component: String, message: String },
}

/// ExEx (Execution Extension) integration errors.
#[derive(Error, Debug)]
pub enum ExExError {
    /// ExEx installation failure
    #[error("ExEx installation failed: {name} - {message}")]
    InstallationFailed { name: String, message: String },

    /// ExEx notification processing error
    #[error("ExEx notification processing error: {message}")]
    NotificationProcessing { message: String },

    /// ExEx context error
    #[error("ExEx context error: {message}")]
    Context { message: String },

    /// ExEx lifecycle error
    #[error("ExEx lifecycle error: {phase} - {message}")]
    Lifecycle { phase: String, message: String },

    /// ExEx communication error
    #[error("ExEx communication error: {message}")]
    Communication { message: String },

    /// ExEx buffer overflow
    #[error("ExEx buffer overflow: {buffer_type} - size: {size}, capacity: {capacity}")]
    BufferOverflow { buffer_type: String, size: usize, capacity: usize },

    /// ExEx reorg handling error
    #[error("ExEx reorg handling error: depth {depth} - {message}")]
    ReorgHandling { depth: u64, message: String },
}

impl RollupError {
    /// Determines if the error is recoverable and suggests a recovery strategy.
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            RollupError::Service(ServiceError::HealthCheckFailed { component, .. }) => {
                RecoveryStrategy::Retry {
                    max_attempts: 3,
                    delay_ms: 5000,
                    description: format!("Retry health check for {}", component),
                }
            }
            RollupError::Service(ServiceError::CommunicationError { .. }) => {
                RecoveryStrategy::Retry {
                    max_attempts: 5,
                    delay_ms: 1000,
                    description: "Retry service communication".to_string(),
                }
            }
            RollupError::Service(ServiceError::InitializationFailed { .. }) => {
                RecoveryStrategy::Fatal(
                    "Service initialization failed - check configuration and restart".to_string(),
                )
            }
            RollupError::ExEx(ExExError::BufferOverflow { .. }) => {
                RecoveryStrategy::Backpressure("Apply backpressure to reduce load".to_string())
            }
            RollupError::ExEx(ExExError::ReorgHandling { depth, .. }) => {
                if *depth > 100 {
                    RecoveryStrategy::Fatal(
                        "Deep reorg detected - manual intervention required".to_string(),
                    )
                } else {
                    RecoveryStrategy::Retry {
                        max_attempts: 1,
                        delay_ms: 0,
                        description: "Retry reorg handling".to_string(),
                    }
                }
            }
            RollupError::ExEx(ExExError::NotificationProcessing { .. }) => {
                RecoveryStrategy::Retry {
                    max_attempts: 3,
                    delay_ms: 2000,
                    description: "Retry notification processing".to_string(),
                }
            }
            RollupError::Service(ServiceError::ShutdownError { .. }) => {
                RecoveryStrategy::ForceShutdown("Force shutdown - timeout exceeded".to_string())
            }
            _ => RecoveryStrategy::Log("Log error and continue".to_string()),
        }
    }

    /// Returns true if the error indicates a fatal condition requiring restart.
    pub fn is_fatal(&self) -> bool {
        matches!(
            self.recovery_strategy(),
            RecoveryStrategy::Fatal(_) | RecoveryStrategy::ForceShutdown(_)
        )
    }

    /// Returns true if the error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(self.recovery_strategy(), RecoveryStrategy::Retry { .. })
    }
}

/// Recovery strategy for handling different types of errors.
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry the operation with specified parameters
    Retry { max_attempts: u32, delay_ms: u64, description: String },
    /// Apply backpressure to reduce load
    Backpressure(String),
    /// Restart the service or component
    Restart(String),
    /// Fatal error requiring manual intervention
    Fatal(String),
    /// Force shutdown immediately
    ForceShutdown(String),
    /// Log error and continue operation
    Log(String),
}

impl RecoveryStrategy {
    /// Returns a human-readable description of the recovery strategy.
    pub fn description(&self) -> &str {
        match self {
            RecoveryStrategy::Retry { description, .. } => description,
            RecoveryStrategy::Backpressure(desc) => desc,
            RecoveryStrategy::Restart(desc) => desc,
            RecoveryStrategy::Fatal(desc) => desc,
            RecoveryStrategy::ForceShutdown(desc) => desc,
            RecoveryStrategy::Log(desc) => desc,
        }
    }
}

/// Result type alias for rollup operations.
pub type RollupResult<T> = Result<T, RollupError>;

// Convenience constructors
impl ServiceError {
    /// Create an initialization failed error.
    pub fn init_failed(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InitializationFailed { component: component.into(), message: message.into() }
    }

    /// Create a startup failed error.
    pub fn startup_failed(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self::StartupFailed { component: component.into(), message: message.into() }
    }

    /// Create a runtime error.
    pub fn runtime_error(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self::RuntimeError { component: component.into(), message: message.into() }
    }

    /// Create a health check failed error.
    pub fn health_check_failed(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self::HealthCheckFailed { component: component.into(), message: message.into() }
    }
}

impl ExExError {
    /// Create an installation failed error.
    pub fn installation_failed(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InstallationFailed { name: name.into(), message: message.into() }
    }

    /// Create a notification processing error.
    pub fn notification_processing(message: impl Into<String>) -> Self {
        Self::NotificationProcessing { message: message.into() }
    }

    /// Create a context error.
    pub fn context(message: impl Into<String>) -> Self {
        Self::Context { message: message.into() }
    }

    /// Create a lifecycle error.
    pub fn lifecycle(phase: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Lifecycle { phase: phase.into(), message: message.into() }
    }

    /// Create a reorg handling error.
    pub fn reorg_handling(depth: u64, message: impl Into<String>) -> Self {
        Self::ReorgHandling { depth, message: message.into() }
    }

    /// Create a communication error.
    pub fn communication(message: impl Into<String>) -> Self {
        Self::Communication { message: message.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_error_recovery_strategy() {
        let error = RollupError::Service(ServiceError::HealthCheckFailed {
            component: "reth".to_string(),
            message: "connection failed".to_string(),
        });

        assert!(!error.is_fatal());
        assert!(error.is_retryable());

        if let RecoveryStrategy::Retry { max_attempts, delay_ms, .. } = error.recovery_strategy() {
            assert_eq!(max_attempts, 3);
            assert_eq!(delay_ms, 5000);
        } else {
            panic!("Expected retry strategy");
        }
    }

    #[test]
    fn test_exex_buffer_overflow_recovery() {
        let error = RollupError::ExEx(ExExError::BufferOverflow {
            buffer_type: "notification".to_string(),
            size: 1000,
            capacity: 800,
        });

        assert!(!error.is_fatal());
        assert!(!error.is_retryable());
        assert!(matches!(error.recovery_strategy(), RecoveryStrategy::Backpressure(_)));
    }

    #[test]
    fn test_deep_reorg_fatal_recovery() {
        let error = RollupError::ExEx(ExExError::ReorgHandling {
            depth: 150,
            message: "reorg too deep".to_string(),
        });

        assert!(error.is_fatal());
        assert!(!error.is_retryable());
        assert!(matches!(error.recovery_strategy(), RecoveryStrategy::Fatal(_)));
    }

    #[test]
    fn test_shallow_reorg_retry_recovery() {
        let error = RollupError::ExEx(ExExError::ReorgHandling {
            depth: 5,
            message: "reorg handling failed".to_string(),
        });

        assert!(!error.is_fatal());
        assert!(error.is_retryable());

        if let RecoveryStrategy::Retry { max_attempts, delay_ms, .. } = error.recovery_strategy() {
            assert_eq!(max_attempts, 1);
            assert_eq!(delay_ms, 0);
        } else {
            panic!("Expected retry strategy");
        }
    }

    #[test]
    fn test_convenience_constructors() {
        let init_error = ServiceError::init_failed("test", "failed");
        assert!(matches!(init_error, ServiceError::InitializationFailed { .. }));

        let startup_error = ServiceError::startup_failed("test", "failed");
        assert!(matches!(startup_error, ServiceError::StartupFailed { .. }));

        let runtime_error = ServiceError::runtime_error("test", "failed");
        assert!(matches!(runtime_error, ServiceError::RuntimeError { .. }));

        let health_error = ServiceError::health_check_failed("test", "failed");
        assert!(matches!(health_error, ServiceError::HealthCheckFailed { .. }));
    }

    #[test]
    fn test_exex_convenience_constructors() {
        let install_error = ExExError::installation_failed("test", "failed");
        assert!(matches!(install_error, ExExError::InstallationFailed { .. }));

        let notification_error = ExExError::notification_processing("failed");
        assert!(matches!(notification_error, ExExError::NotificationProcessing { .. }));

        let context_error = ExExError::context("failed");
        assert!(matches!(context_error, ExExError::Context { .. }));

        let lifecycle_error = ExExError::lifecycle("init", "failed");
        assert!(matches!(lifecycle_error, ExExError::Lifecycle { .. }));
    }
}
