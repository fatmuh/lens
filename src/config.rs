//! Configuration loading and the `init` subcommand.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::InitArgs;

pub const CONFIG_FILENAME: &str = "quality-gate.toml";
pub const LENSIGNORE_FILENAME: &str = ".lensignore";
pub const EXAMPLE_CONFIG: &str = include_str!("../quality-gate.toml.example");

/// Top-level configuration. All sections are optional; defaults applied below.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub scan: ScanConfig,
    pub nosonar: NosonarConfig,
    pub duplication: DuplicationConfig,
    pub coverage: CoverageConfig,
    pub issues: IssuesConfig,
    pub rules: RulesConfig,
    pub output: OutputConfig,
    pub html: HtmlConfig,
    pub watch: WatchConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanConfig {
    /// Root directory for resolving relative paths.
    pub root: PathBuf,
    /// Glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Optional whitelist. If non-empty, files MUST match at least one.
    pub include: Vec<String>,
    /// Max file size in bytes.
    pub max_file_size_bytes: u64,
    /// Number of parallel jobs (0 = auto).
    pub parallel_jobs: usize,
    /// Respect .gitignore.
    pub respect_gitignore: bool,
    /// Respect .lensignore.
    pub respect_lensignore: bool,
    /// Path to lens-specific ignore file (relative to root).
    pub ignore_file: String,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            exclude: Vec::new(),
            include: Vec::new(),
            max_file_size_bytes: 1024 * 1024,
            parallel_jobs: 0,
            respect_gitignore: true,
            respect_lensignore: true,
            ignore_file: LENSIGNORE_FILENAME.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NosonarConfig {
    pub enabled: bool,
    pub custom_markers: Vec<String>,
}

impl Default for NosonarConfig {
    fn default() -> Self {
        Self { enabled: true, custom_markers: Vec::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DuplicationConfig {
    /// Algorithm to use: "token" (default, sensitive) or "sonar" (line-based,
    /// SonarQube-compatible).
    pub mode: String,
    /// Minimum block size in tokens (used by `mode = "token"`).
    pub min_tokens: usize,
    /// Minimum block size in lines (used by `mode = "sonar"`).
    /// SonarQube's default is 100.
    pub min_lines: usize,
    /// In sonar mode, normalize identifiers to `@id` before hashing
    /// (catches renamed-variable duplicates, like SonarQube does).
    pub normalize_identifiers: bool,
    pub fail_above_percent: f64,
}

impl Default for DuplicationConfig {
    fn default() -> Self {
        Self {
            mode: "token".to_string(),
            min_tokens: 100,
            min_lines: 100,
            normalize_identifiers: false,
            fail_above_percent: 3.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CoverageConfig {
    pub report_paths: Vec<String>,
    pub fail_below_percent: f64,
    pub exclude: Vec<String>,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            report_paths: vec!["coverage/lcov.info".into()],
            fail_below_percent: 80.0,
            exclude: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IssuesConfig {
    pub fail_on: Vec<String>,
    pub max_blocker: i64,
    pub max_critical: i64,
    pub max_major: i64,
    pub max_minor: i64,
    pub max_info: i64,
}

impl Default for IssuesConfig {
    fn default() -> Self {
        Self {
            fail_on: vec!["blocker".into(), "critical".into()],
            max_blocker: 0,
            max_critical: 0,
            max_major: -1,
            max_minor: -1,
            max_info: -1,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RulesConfig {
    pub disabled: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub format: String,
    pub gate_fail_exit_code: i32,
    pub color: String,
    pub show_source: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: "terminal".into(),
            gate_fail_exit_code: 1,
            color: "auto".into(),
            show_source: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HtmlConfig {
    pub output: PathBuf,
    pub open_browser: bool,
}

impl Default for HtmlConfig {
    fn default() -> Self {
        Self { output: PathBuf::from("lens-report"), open_browser: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchConfig {
    pub debounce_ms: u64,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self { debounce_ms: 200 }
    }
}

impl Config {
    /// Load config from a path. Falls back to defaults if the file doesn't exist.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };

        if !path.exists() {
            anyhow::bail!("config file not found: {}", path.display());
        }

        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config: {}", path.display()))?;
        let cfg: Self = toml::from_str(&text)
            .with_context(|| format!("parsing config: {}", path.display()))?;
        Ok(cfg)
    }

    /// Resolve the actual config path: explicit path > quality-gate.toml in scan root.
    pub fn resolve_path(explicit: Option<&Path>, scan_root: &Path) -> Option<PathBuf> {
        if let Some(p) = explicit {
            return Some(p.to_path_buf());
        }
        let candidate = scan_root.join(CONFIG_FILENAME);
        if candidate.exists() { Some(candidate) } else { None }
    }
}

/// `lens init` — write a starter `quality-gate.toml` to the target directory.
pub fn init(args: InitArgs) -> Result<std::process::ExitCode> {
    let target = args.path.join(CONFIG_FILENAME);
    if target.exists() && !args.force {
        anyhow::bail!(
            "{} already exists. Pass --force to overwrite.",
            target.display()
        );
    }
    std::fs::write(&target, EXAMPLE_CONFIG)
        .with_context(|| format!("writing {}", target.display()))?;
    println!("✓ Created {}", target.display());
    println!("\nNext steps:");
    println!("  1. Edit {} to fit your project", CONFIG_FILENAME);
    println!("  2. Run: lens scan .");
    Ok(std::process::ExitCode::SUCCESS)
}
