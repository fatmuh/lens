//! Lens — Lightweight code quality scanner.
//!
//! Entry point. Wires up logging, CLI parsing, and dispatches to subcommands.

use std::process::ExitCode;

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use tracing_subscriber::EnvFilter;

mod analyzer;
mod agent;
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
        Command::Fix(args) => {
            let config = agent::client::AiConfig {
                api_key: std::env::var("LENS_AI_API_KEY")
                    .context("LENS_AI_API_KEY is required. Set it or use: $env:LENS_AI_API_KEY = \"sk-...\"")?,
                base_url: args.ai_base_url.clone(),
                model: args.ai_model.clone(),
            };
            let root = args.path.canonicalize().context("resolving path")?;
            let rt = tokio::runtime::Runtime::new().context("creating runtime")?;
            println!("\n  {} Lens AI Fix Agent", "🤖".to_string().cyan());
            println!("  {} Model: {}", "→".to_string().dimmed(), config.model);

            match args.mode {
                cli::AgentMode::Coverage | cli::AgentMode::All => {
                    if let Some(lcov) = &args.coverage {
                        rt.block_on(agent::coverage::fix_uncovered(
                            &config, &root, lcov, args.max_files,
                        ))?;
                    } else {
                        println!("  {} No --coverage specified, skipping coverage agent", "→".to_string().dimmed());
                    }
                }
                _ => {}
            }
            match args.mode {
                cli::AgentMode::Dedup | cli::AgentMode::All => {
                    rt.block_on(agent::dedup::fix_duplicates(
                        &config, &root, 20, args.max_files,
                    ))?;
                }
                _ => {}
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Watch(args) => {
            let config = agent::client::AiConfig {
                api_key: std::env::var("LENS_AI_API_KEY")
                    .context("LENS_AI_API_KEY is required")?,
                base_url: args.ai_base_url.clone(),
                model: args.ai_model.clone(),
            };
            let mode = match args.mode {
                cli::AgentMode::Coverage => agent::watch::AgentMode::Coverage,
                cli::AgentMode::Dedup => agent::watch::AgentMode::Dedup,
                cli::AgentMode::All => agent::watch::AgentMode::All,
            };
            println!("\n  {} Lens AI Watch Agent", "👁".to_string().cyan());
            println!("  {} Model: {}", "→".to_string().dimmed(), config.model);
            agent::watch::watch(
                &args.path.canonicalize().context("resolving path")?,
                &config,
                mode,
                args.coverage.as_deref(),
                args.debounce_ms,
            )?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn rustc_version_runtime() -> &'static str {
    // Compile-time known, no runtime cost.
    env!("CARGO_PKG_RUST_VERSION", "unknown")
}
