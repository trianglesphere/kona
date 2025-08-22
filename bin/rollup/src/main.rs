//! Unified rollup binary with kona-node ExEx integration.

#![deny(missing_docs, unused_must_use, rust_2018_idioms)]

pub mod cli;
pub mod exex;
pub mod flags;
pub mod node_builder;

fn main() {
    use clap::Parser;

    kona_cli::sigsegv_handler::install();
    kona_cli::backtrace::enable();

    if let Err(err) = cli::Cli::parse().run() {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}
