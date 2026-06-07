//! Command-line interface definitions (clap derive).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "lens",
    version,
    about = "Lightweight code quality scanner — issues, duplication, coverage",
    long_about = "Lens scans your project for code issues, duplication, and coverage. \
                  It honors `.gitignore`/`.lensignore`, supports include/exclude patterns, \
                  and respects `NOSONAR` comments.",
    propagate_version = true,
)]
pub struct Cli {
    /// Path to config file (default: `quality-gate.toml` in scan root or current dir).
    #[arg(long, short = 'c', global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::try_parse().unwrap_or_else(|e| {
            // clap has its own nicely formatted help/error writer; let it print
            // directly to stdout/stderr and exit before we get involved.
            e.exit()
        })
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scan a directory for code quality issues.
    #[command(visible_alias = "s")]
    Scan(ScanArgs),

    /// Initialize a `quality-gate.toml` in the current directory.
    #[command(visible_alias = "i")]
    Init(InitArgs),

    /// List all available rules.
    Rules,

    /// Print version information.
    Version,
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    /// Directory to scan (default: current dir).
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Output format.
    #[arg(long, short = 'f', value_enum, default_value_t = Format::Terminal)]
    pub format: Format,

    /// Output file (default: stdout for terminal/json, dir for html).
    #[arg(long, short = 'o', value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Apply quality gate — exit with non-zero code on failure.
    #[arg(long)]
    pub gate: bool,

    /// Watch for file changes and re-scan automatically.
    #[arg(long, short = 'w')]
    pub watch: bool,

    /// Show verbose progress.
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Do not respect `.gitignore`.
    #[arg(long)]
    pub no_gitignore: bool,

    /// Skip analysis (only do discovery / config check). Useful for `lens init` flow.
    #[arg(long)]
    pub dry_run: bool,

    /// Suppress progress output and non-essential decorations.
    /// Useful in CI environments.
    #[arg(long, short = 'q')]
    pub quiet: bool,

    /// Disable colored output (also honors the `NO_COLOR` env var).
    #[arg(long)]
    pub no_color: bool,

    /// Use SonarQube-compatible (line-based) duplication detection.
    /// Overrides `duplication.mode` in the config file.
    #[arg(long)]
    pub sonar_compat: bool,

    /// Minimum block size for `--sonar-compat` mode (default: 250).
    /// SonarQube's effective default for typical TypeScript code
    /// (verified: 2.51% vs office's 2.5% on pos-glid-b2b, < 0.02% diff).
    /// Lower values (e.g. 100–200) are more sensitive, higher values
    /// (500+) only flag the biggest duplicates.
    #[arg(long)]
    pub min_duplicate_lines: Option<usize>,

    /// In `--sonar-compat` mode, normalize identifiers (variable, function,
    /// class names) to `@id` before hashing. This makes the algorithm
    /// invariant to renames, matching SonarQube's "duplications" metric
    /// more closely. Off by default to preserve exact-hash semantics.
    #[arg(long)]
    pub normalize_identifiers: bool,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Overwrite existing config file.
    #[arg(long)]
    pub force: bool,

    /// Target directory (default: current dir).
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Format {
    /// Human-readable terminal output.
    Terminal,
    /// Machine-readable JSON.
    Json,
    /// HTML report (single page).
    Html,
    /// SARIF (for GitHub code scanning etc.).
    Sarif,
}

impl Format {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Format::Terminal => "terminal",
            Format::Json => "json",
            Format::Html => "html",
            Format::Sarif => "sarif",
        }
    }
}
