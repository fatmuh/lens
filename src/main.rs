//! Lens — Lightweight code quality scanner.
//!
//! Entry point. Wires up logging, CLI parsing, and dispatches to subcommands.

use std::process::ExitCode;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

mod analyzer;
mod cli;
mod config;
mod coverage;
mod report;
mod rules;
mod scanner;
mod state;
mod util;
mod rating;

use cli::{Cli, Command};

fn main() -> ExitCode {
    // Load .env-like LOG env var: e.g. RUST_LOG=lens=debug
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse_args();

    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            tracing::error!("{e:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Scan(args) => scanner::run(cli.config, args),
        Command::Init(args) => config::init(args),
        Command::Rules(args) => scanner::rules::list(args),
        Command::Version => {
            println!("lens {}", env!("CARGO_PKG_VERSION"));
            println!("rust  {}", rustc_version_runtime());
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn rustc_version_runtime() -> &'static str {
    // Compile-time known, no runtime cost.
    env!("CARGO_PKG_RUST_VERSION", "unknown")
}
