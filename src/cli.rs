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
    propagate_version = true
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

    /// Run tests with coverage and optionally fix gaps with AI.
    #[command(visible_alias = "t")]
    Test(TestArgs),

    /// Print version information.
    Version,

    /// Run CI scan — SARIF output + quality gate + PR comment.
    #[command(visible_alias = "c")]
    Ci(CiArgs),

    /// Self-update to the latest release from GitHub.
    #[command(visible_alias = "u")]
    Update(UpdateArgs),

    /// Scan dependencies for known vulnerabilities (OSV database).
    #[command(visible_alias = "d")]
    Dep(DepArgs),

    /// Dynamic security scan using OWASP ZAP.
    #[command(visible_alias = "z")]
    Zap(ZapArgs),
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

    /// Use token-based (shingling + winnowing) duplication detection.
    /// More sensitive than the default SonarQube-compatible mode.
    /// Produces higher duplication percentages (catches smaller blocks).
    #[arg(long)]
    pub token_mode: bool,

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
    #[arg(
        long,
        env = "LENS_AI_BASE_URL",
        default_value = "https://api.openai.com/v1"
    )]
    pub ai_base_url: String,

    /// AI model to use.
    #[arg(long, env = "LENS_AI_MODEL", default_value = "gpt-4o")]
    pub ai_model: String,

    /// Show diff preview without writing files.
    #[arg(long)]
    pub dry_run: bool,
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
    #[arg(
        long,
        env = "LENS_AI_BASE_URL",
        default_value = "https://api.openai.com/v1"
    )]
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

#[derive(Debug, Args)]
pub struct TestArgs {
    /// Directory to test (default: current dir).
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Run with coverage (generates LCOV report).
    #[arg(long, default_value_t = true)]
    pub coverage: bool,

    /// After running tests, feed coverage gaps to AI agent.
    #[arg(long, short = 'f', default_value_t = false)]
    pub fix: bool,

    /// Agent mode when fixing (only used with --fix).
    #[arg(long, short = 'm', value_enum, default_value_t = AgentMode::Coverage)]
    pub mode: AgentMode,

    /// Maximum number of files to fix per run.
    #[arg(long, default_value_t = 5)]
    pub max_files: usize,

    /// Only detect the test framework, don't run.
    #[arg(long, default_value_t = false)]
    pub detect_only: bool,

    /// OpenAI-compatible API base URL (overrides ~/.lens/config.toml).
    #[arg(long, env = "LENS_AI_BASE_URL")]
    pub ai_base_url: Option<String>,

    /// AI model to use (overrides ~/.lens/config.toml).
    #[arg(long, env = "LENS_AI_MODEL")]
    pub ai_model: Option<String>,

    /// Show diff preview without writing files.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Only check for updates — don't download or install.
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Args)]
pub struct DepArgs {
    /// Directory to scan for dependency files (default: current dir).
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: String,

    /// Only list dependencies without querying OSV.
    #[arg(long)]
    pub audit_only: bool,

    /// Exit with non-zero code if critical/high vulnerabilities found.
    #[arg(long)]
    pub gate: bool,

    /// Output format.
    #[arg(long, short = 'f', value_enum, default_value_t = crate::cli::Format::Terminal)]
    pub format: crate::cli::Format,
}

#[derive(Debug, Args)]
pub struct ZapArgs {
    /// Target URL to scan (e.g. http://localhost:3000).
    #[arg(value_name = "URL")]
    pub target: String,

    /// ZAP API host (default: http://127.0.0.1:8080).
    #[arg(long)]
    pub zap_host: Option<String>,

    /// ZAP API key (default: empty for local).
    #[arg(long)]
    pub zap_key: Option<String>,

    /// ZAP Docker image (default: zaproxy/zap-stable).
    #[arg(long, default_value = "zaproxy/zap-stable")]
    pub zap_image: String,

    /// Port for ZAP container (default: 8080).
    #[arg(long)]
    pub zap_port: Option<u16>,

    /// Don't start a Docker container — connect to existing ZAP.
    #[arg(long)]
    pub no_docker: bool,

    /// Use AJAX spider for SPA applications.
    #[arg(long)]
    pub ajax: bool,

    /// Spider max depth (default: 5).
    #[arg(long)]
    pub max_depth: Option<u32>,

    /// Timeout per phase in seconds (default: 120).
    #[arg(long, default_value_t = 120)]
    pub timeout: u64,

    /// Exit with non-zero code if high/medium vulnerabilities found.
    #[arg(long)]
    pub gate: bool,

    /// Output format.
    #[arg(long, short = 'f', value_enum, default_value_t = crate::cli::Format::Terminal)]
    pub format: crate::cli::Format,
}

#[derive(Debug, Args)]
pub struct CiArgs {
    /// Directory to scan (default: current dir).
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Output SARIF file path (default: lens-results.sarif).
    #[arg(
        long,
        short = 'o',
        value_name = "FILE",
        default_value = "lens-results.sarif"
    )]
    pub output: PathBuf,

    /// Fail on quality gate violation (exit 1).
    #[arg(long)]
    pub gate: bool,

    /// Maximum rating allowed (A-E). Fails if any rating is worse.
    #[arg(long = "max-rating", value_name = "LETTER", value_parser = parse_rating)]
    pub max_rating: Option<crate::rating::Rating>,

    /// Print a GitHub PR comment body to stdout.
    #[arg(long)]
    pub pr_comment: bool,

    /// Only show issues from files modified within the last N days.
    #[arg(long = "since-days", value_name = "DAYS")]
    pub since_days: Option<u32>,

    /// Only show issues from files added/changed since previous scan.
    #[arg(long)]
    pub new_code: bool,
}
