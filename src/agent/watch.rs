//! File watcher that triggers AI agents on change.
//!
//! Watches the project directory for file changes. When a file changes:
//!   1. Runs a quick scan to detect new coverage gaps and duplicates
//!   2. Spawns the appropriate agent (coverage or dedup)
//!   3. Reports what was fixed
//!
//! The watcher respects .gitignore and .lensignore.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::client::AiConfig;
use super::coverage;
use super::dedup;

/// Agent mode — which agent(s) to run.
#[derive(Debug, Clone, Copy)]
pub enum AgentMode {
    /// Generate tests for uncovered lines.
    Coverage,
    /// Refactor duplicated code.
    Dedup,
    /// Run both agents.
    All,
}

/// Run the watch loop.
pub fn watch(
    project_root: &Path,
    config: &AiConfig,
    mode: AgentMode,
    lcov_path: Option<&Path>,
    debounce_ms: u64,
) -> Result<()> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

    let root = project_root.canonicalize().context("resolving project root")?;
    println!("  {} Watching {} for changes...", "👁".to_string().cyan(), root.display());
    println!("  {} Press Ctrl+C to stop", "→".to_string().dimmed());

    // Set up file watcher.
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )?;

    watcher.watch(&root, RecursiveMode::Recursive)?;

    let mut last_run = Instant::now() - Duration::from_millis(debounce_ms);
    let debounce = Duration::from_millis(debounce_ms);

    // Create a minimal Tokio runtime for async agent calls.
    let rt = tokio::runtime::Runtime::new().context("creating tokio runtime")?;

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => {
                // Only react to file content changes (not metadata/creates/deletes).
                let relevant = event.kind.is_modify()
                    && event.paths.iter().any(|p| {
                        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                        matches!(ext, "ts" | "tsx" | "js" | "jsx")
                    });

                if !relevant { continue; }

                // Debounce: wait for a quiet period after last change.
                let now = Instant::now();
                if now.duration_since(last_run) < debounce {
                    continue;
                }

                // Drain any pending events (debounce window).
                while rx.recv_timeout(Duration::from_millis(200)).is_ok() {}

                println!(
                    "\n  {} Change detected at {}",
                    "⚡".to_string().yellow(),
                    chrono::Local::now().format("%H:%M:%S"),
                );

                run_agents(&rt, config, project_root, mode, lcov_path);
                last_run = Instant::now();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Normal — no changes.
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

fn run_agents(
    rt: &tokio::runtime::Runtime,
    config: &AiConfig,
    project_root: &Path,
    mode: AgentMode,
    lcov_path: Option<&Path>,
) {
    match mode {
        AgentMode::Coverage | AgentMode::All => {
            if let Some(lcov) = lcov_path {
                println!("\n  {} Coverage agent starting...", "🧪".to_string().cyan());
                match rt.block_on(coverage::fix_uncovered(config, project_root, lcov, 5)) {
                    Ok(result) => {
                        println!(
                            "  {} Covered {} lines across {} test files",
                            "✓".green(),
                            result.lines_covered,
                            result.test_files_written.len(),
                        );
                    }
                    Err(e) => eprintln!("  {} Coverage agent error: {}", "✗".red(), e),
                }
            }
        }
        _ => {}
    }

    match mode {
        AgentMode::Dedup | AgentMode::All => {
            println!("\n  {} Dedup agent starting...", "♻".to_string().cyan());
            match rt.block_on(dedup::fix_duplicates(config, project_root, 20, 3)) {
                Ok(result) => {
                    println!(
                        "  {} Refactored {} duplicate groups, modified {} files",
                        "✓".green(),
                        result.blocks_refactored,
                        result.files_modified.len() + result.shared_files_created.len(),
                    );
                }
                Err(e) => eprintln!("  {} Dedup agent error: {}", "✗".red(), e),
            }
        }
        _ => {}
    }
}

use owo_colors::OwoColorize;
