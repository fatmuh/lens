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
    Rules(RulesArgs),

    /// AI-powered auto-fix: generate tests for uncovered lines and refactor duplicates.
    #[command(visible_alias = "f")]
    Fix(FixArgs),

    /// Watch for file changes and auto-fix with AI.
    #[command(visible_alias = "w")]
    Watch(WatchArgs),

    /// Configure AI settings (API key, model, base URL).
    #[command(visible_alias = "cfg")]
    Setup,

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

    /// Glob for unit-test coverage reports (LCOV, Cobertura, JaCoCo).
    /// Can be specified multiple times. Merged with `coverage.report_paths`
    /// in config under the "ut" category.
    #[arg(long = "coverage-ut", value_name = "GLOB")]
    pub coverage_ut: Vec<String>,

    /// Glob for integration-test coverage reports. Same formats as
    /// `--coverage-ut`. Tracked separately as `it_coverage` and
    /// `overall_coverage` in the output.
    #[arg(long = "coverage-it", value_name = "GLOB")]
    pub coverage_it: Vec<String>,

    /// Only show issues from files added/changed since the previous scan
    /// (requires `.lens/state.json` from a prior run).
    #[arg(long)]
    pub new_code: bool,

    /// Don't save a new state snapshot to `.lens/state.json` after this
    /// scan. Useful for read-only scans (CI, dry-runs).
    #[arg(long)]
    pub no_state: bool,

    /// Only show issues from files modified within the last N days.
    /// Uses file mtime (no state needed). If `--new-code` is also set,
    /// the two filters combine with OR logic.
    #[arg(long = "since-days", value_name = "DAYS")]
    pub since_days: Option<u32>,

    /// Fail the quality gate if any of (reliability, security,
    /// maintainability) is worse than this letter. E.g. `--max-rating C`
    /// fails on D or E. Default: don't gate on ratings.
    #[arg(long = "max-rating", value_name = "LETTER", value_parser = parse_rating)]
    pub max_rating: Option<crate::rating::Rating>,
}

fn parse_rating(s: &str) -> Result<crate::rating::Rating, String> {
    use crate::rating::Rating::*;
    match s.to_ascii_uppercase().as_str() {
        "A" => Ok(A),
        "B" => Ok(B),
        "C" => Ok(C),
        "D" => Ok(D),
        "E" => Ok(E),
        _ => Err(format!("invalid rating '{}', expected A|B|C|D|E", s)),
    }
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

#[derive(Debug, Args)]
pub struct RulesArgs {
    /// Show full descriptions (one per rule).
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Filter by language (e.g. typescript, rust).
    #[arg(long, value_name = "LANG")]
    pub language: Option<String>,

    /// Output format.
    #[arg(long, short = 'f', value_enum, default_value_t = Format::Terminal)]
    pub format: Format,
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


#[derive(Debug, Args)]
pub struct FixArgs {
    /// Directory to fix (default: current dir).
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Agent mode: what to fix.
    #[arg(long, short = 'm', value_enum, default_value_t = AgentMode::All)]
    pub mode: AgentMode,

    /// Path to LCOV coverage report (required for coverage agent).
    #[arg(long, value_name = "PATH")]
    pub coverage: Option<PathBuf>,

    /// Maximum number of files/blocks to process per run.
    #[arg(long, default_value_t = 5)]
    pub max_files: usize,

    /// OpenAI-compatible API base URL.
    #[arg(long, env = "LENS_AI_BASE_URL", default_value = "https://api.openai.com/v1")]
    pub ai_base_url: String,

    /// AI model to use.
    #[arg(long, env = "LENS_AI_MODEL", default_value = "gpt-4o")]
    pub ai_model: String,
}

#[derive(Debug, Args)]
pub struct WatchArgs {
    /// Directory to watch (default: current dir).
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Agent mode: what to fix on change.
    #[arg(long, short = 'm', value_enum, default_value_t = AgentMode::All)]
    pub mode: AgentMode,

    /// Path to LCOV coverage report (required for coverage agent).
    #[arg(long, value_name = "PATH")]
    pub coverage: Option<PathBuf>,

    /// Debounce interval in milliseconds.
    #[arg(long, default_value_t = 2000)]
    pub debounce_ms: u64,

    /// OpenAI-compatible API base URL.
    #[arg(long, env = "LENS_AI_BASE_URL", default_value = "https://api.openai.com/v1")]
    pub ai_base_url: String,

    /// AI model to use.
    #[arg(long, env = "LENS_AI_MODEL", default_value = "gpt-4o")]
    pub ai_model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AgentMode {
    Coverage,
    Dedup,
    All,
}
