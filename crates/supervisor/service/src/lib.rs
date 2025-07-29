//! This crate provides the runnable service layer for the Kona Supervisor.
//! It integrates the core logic with the RPC server.
#![deny(unused_crate_dependencies)]

mod service;

pub use service::Service;

mod actors;
pub use actors::SupervisorActor;
