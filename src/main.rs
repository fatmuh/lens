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
mod setup;
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
        Command::Setup => setup::run_setup(),
        Command::Test(args) => run_test_command(args),
        Command::Fix(args) => {
            let mut base_url = args.ai_base_url.clone();
            let mut model = args.ai_model.clone();
            let api_key = if let Ok(k) = std::env::var("LENS_AI_API_KEY") {
                k
            } else if let Ok(cfg) = setup::load_config() {
                // Fallback to ~/.lens/config.toml
                if base_url == "https://api.openai.com/v1" && !cfg.ai.base_url.is_empty() {
                    base_url = cfg.ai.base_url.clone();
                }
                if model == "gpt-4o" && !cfg.ai.model.is_empty() {
                    model = cfg.ai.model.clone();
                }
                cfg.ai.api_key.clone()
            } else {
                anyhow::bail!("LENS_AI_API_KEY not set. Run `lens setup` to configure AI.");
            };
            let config = agent::client::AiConfig {
                api_key,
                base_url: base_url.clone(),
                model: model.clone(),
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
            let mut base_url = args.ai_base_url.clone();
            let mut model = args.ai_model.clone();
            let api_key = if let Ok(k) = std::env::var("LENS_AI_API_KEY") {
                k
            } else if let Ok(cfg) = setup::load_config() {
                if base_url == "https://api.openai.com/v1" && !cfg.ai.base_url.is_empty() {
                    base_url = cfg.ai.base_url.clone();
                }
                if model == "gpt-4o" && !cfg.ai.model.is_empty() {
                    model = cfg.ai.model.clone();
                }
                cfg.ai.api_key.clone()
            } else {
                anyhow::bail!("LENS_AI_API_KEY not set. Run `lens setup` to configure AI.");
            };
            let config = agent::client::AiConfig {
                api_key,
                base_url: base_url.clone(),
                model: model.clone(),
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

fn run_test_command(args: cli::TestArgs) -> Result<ExitCode> {
    use owo_colors::OwoColorize;

    let root = args.path.canonicalize().context("resolving path")?;

    // 1. Detect test framework.
    println!("\n  {} Detecting test framework...", "🔍".to_string().cyan());
    let framework = match agent::test_runner::detect(&root) {
        Ok(f) => f,
        Err(e) => {
            println!("  {} {}", "✗".red(), e);
            return Ok(ExitCode::from(1));
        }
    };

    println!("  {} Detected: {}", "✓".green().bold(), framework.name.cyan());
    if let Some(ref cfg) = framework.config_file {
        println!("  {} Config: {}", "→".dimmed(), cfg);
    }
    println!("  {} Coverage cmd: {}", "→".dimmed(), framework.coverage_cmd.join(" ").cyan());

    if args.detect_only {
        println!();
        return Ok(ExitCode::SUCCESS);
    }

    // 2. Run tests with coverage.
    println!("\n  {} Running tests with coverage...", "🧪".to_string().cyan());
    let result = agent::test_runner::run_with_coverage(&root, &framework)?;

    // 3. Print results.
    println!();
    if result.success {
        println!("  {} Tests passed", "✓".green().bold());
    } else {
        println!("  {} Tests FAILED", "✗".red().bold());
    }
    if result.total_tests > 0 {
        println!(
            "  {} passed, {} failed, {} skipped ({} total) in {}ms",
            result.passed.to_string().green(),
            result.failed.to_string().red(),
            result.skipped.to_string().yellow(),
            result.total_tests,
            result.duration_ms,
        );
    }
    if let Some(ref cov_file) = result.coverage_file {
        println!(
            "  {} Coverage: {:.1}% → {}",
            "📊".to_string().cyan(),
            result.coverage_percent,
            cov_file.display(),
        );
    } else {
        println!("  {} No coverage report found in {}", "⚠".yellow(), framework.coverage_output_dir);
    }

    // 4. If --fix, feed to AI agent.
    if args.fix {
        if result.coverage_file.is_none() {
            println!("\n  {} Cannot fix — no coverage report available.", "✗".red());
            return Ok(ExitCode::from(1));
        }

        // Load AI config.
        let mut base_url = args.ai_base_url.unwrap_or_default();
        let mut model = args.ai_model.unwrap_or_default();
        let api_key = if let Ok(k) = std::env::var("LENS_AI_API_KEY") {
            k
        } else if let Ok(cfg) = setup::load_config() {
            if base_url.is_empty() && !cfg.ai.base_url.is_empty() {
                base_url = cfg.ai.base_url.clone();
            }
            if model.is_empty() && !cfg.ai.model.is_empty() {
                model = cfg.ai.model.clone();
            }
            cfg.ai.api_key.clone()
        } else {
            println!("\n  {} AI not configured. Run `lens setup` first.", "✗".red());
            return Ok(ExitCode::from(1));
        };

        if base_url.is_empty() { base_url = "https://api.openai.com/v1".into(); }
        if model.is_empty() { model = "gpt-4o".into(); }

        let ai_config = agent::client::AiConfig { api_key, base_url, model };
        let lcov = result.coverage_file.as_ref().unwrap();

        println!("\n  {} AI Agent starting...", "🤖".to_string().cyan());
        println!("  {} Model: {}", "→".dimmed(), ai_config.model.yellow());

        let rt = tokio::runtime::Runtime::new().context("creating runtime")?;

        match args.mode {
            cli::AgentMode::Coverage | cli::AgentMode::All => {
                let fix_result = rt.block_on(
                    agent::coverage::fix_uncovered(&ai_config, &root, lcov, args.max_files)
                )?;
                println!(
                    "  {} Generated {} test files covering {} lines",
                    "✓".green().bold(),
                    fix_result.test_files_written.len(),
                    fix_result.lines_covered,
                );
                if !fix_result.test_files_written.is_empty() {
                    println!("\n  {} Re-run tests to verify:", "→".bold());
                    println!("    lens test .");
                }
            }
            cli::AgentMode::Dedup => {
                rt.block_on(
                    agent::dedup::fix_duplicates(&ai_config, &root, 20, args.max_files)
                )?;
            }
        }
    }

    if !result.success {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}
