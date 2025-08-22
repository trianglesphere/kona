//! Unified rollup binary combining op-reth and kona-node into a single service.

#![doc(issue_tracker_base_url = "https://github.com/op-rs/kona/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(rust_2018_idioms)]

use rollup::Cli;
use std::process::ExitCode;
use tokio::signal;
use tracing::{error, info};

// TODO: Import from rollup-service crate when available
// use rollup_service::RollupService;

/// Main entry point for the rollup binary.
#[tokio::main]
async fn main() -> ExitCode {
    // Install signal handlers for graceful shutdown
    install_signal_handlers();

    // Parse CLI arguments
    let cli = match Cli::parse_args() {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("Failed to parse CLI arguments: {}", e);
            return ExitCode::from(1);
        }
    };

    // Initialize logging
    if let Err(e) = cli.init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        return ExitCode::from(1);
    }

    // Convert CLI to configuration
    let mut config = match cli.into_config() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to create configuration: {}", e);
            return ExitCode::from(1);
        }
    };

    // Merge environment variables
    if let Err(e) = config.merge_env_vars() {
        error!("Failed to merge environment variables: {}", e);
        return ExitCode::from(1);
    }

    info!("Starting kona rollup binary");

    // TODO: Replace this placeholder with actual rollup-service usage when available
    // let mut service = match RollupService::new(config).await {
    //     Ok(service) => service,
    //     Err(e) => {
    //         error!("Failed to create rollup service: {}", e);
    //         return ExitCode::from(1);
    //     }
    // };
    // 
    // if let Err(e) = service.start().await {
    //     error!("Failed to start rollup service: {}", e);
    //     return ExitCode::from(1);
    // }
    // 
    // tokio::select! {
    //     result = service.run() => {
    //         match result {
    //             Ok(()) => {
    //                 info!("Rollup service completed successfully");
    //                 ExitCode::from(0)
    //             }
    //             Err(e) => {
    //                 error!("Rollup service failed: {}", e);
    //                 ExitCode::from(1)
    //             }
    //         }
    //     }
    //     _ = wait_for_shutdown_signal() => {
    //         info!("Shutdown signal received, initiating graceful shutdown");
    //         if let Err(e) = service.shutdown().await {
    //             error!("Error during shutdown: {}", e);
    //             ExitCode::from(1)
    //         } else {
    //             info!("Graceful shutdown completed");
    //             ExitCode::from(0)
    //         }
    //     }
    // }

    // Placeholder implementation until rollup-service crate is available
    info!("Configuration loaded successfully: {:?}", config.global.chain);
    info!("Waiting for shutdown signal (rollup-service integration pending)");
    
    wait_for_shutdown_signal().await;
    info!("Shutdown signal received");
    
    info!("Kona rollup binary exiting");
    ExitCode::from(0)
}

/// Install signal handlers for graceful shutdown.
fn install_signal_handlers() {
    // Install SIGSEGV handler from kona-cli if available
    kona_cli::sigsegv_handler::install();
    
    // Enable backtrace for better error reporting
    kona_cli::backtrace::enable();
}

/// Wait for shutdown signals (SIGINT, SIGTERM).
async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received SIGINT (Ctrl+C)");
        },
        _ = terminate => {
            info!("Received SIGTERM");
        },
    }
}