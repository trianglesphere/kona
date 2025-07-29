//! Metrics collection and reporting for the supervisor.
#![deny(unused_crate_dependencies)]
mod reporter;
pub use reporter::MetricsReporter;

mod macros;
