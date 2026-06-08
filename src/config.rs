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
    pub significant_code: SignificantCodeConfig,
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
        Self {
            enabled: true,
            custom_markers: Vec::new(),
        }
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
            // SonarQube's effective minimum: although the configuration
            // knob is `sonar.cpd.*.minimumTokens=100` (≈ 10 statements per
            // block × 10 blocks), in practice office SonarQube reports
            // align with our `min_lines=250` for typical TypeScript code
            // (verified on pos-glid-b2b: 2.51% vs office's 2.5%, a
            // difference of < 0.02%). Use 100 for sensitive local scans,
            // 250 to match SonarQube.
            min_lines: 250,
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
    /// Glob for unit-test coverage reports. If set, separate `ut_coverage`
    /// is reported. (LCOV / Cobertura / JaCoCo.)
    #[serde(default)]
    pub ut_paths: Vec<String>,
    /// Glob for integration-test coverage reports. Separate `it_coverage`.
    #[serde(default)]
    pub it_paths: Vec<String>,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            report_paths: vec!["coverage/lcov.info".into()],
            fail_below_percent: 80.0,
            exclude: Vec::new(),
            ut_paths: Vec::new(),
            it_paths: Vec::new(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RulesConfig {
    /// Rule IDs to disable (e.g. `["no-magic-numbers", "no-console"]`).
    pub disabled: Vec<String>,
    /// `max-function-lines` threshold (default 50).
    pub max_function_lines: u32,
    /// `max-function-complexity` cognitive complexity threshold (default 15).
    pub max_function_complexity: u32,
    /// `max-params` threshold (default 5).
    pub max_params: u32,
    /// `no-magic-numbers` minimum |value| to flag (default 3; numbers
    /// with absolute value below this are ignored, in addition to the
    /// built-in allowed list -1, 0, 1, 2, 10, 100, 1000).
    pub no_magic_numbers_min: u32,
    /// Reserved for future `max-nested-depth` rule.
    pub max_nested_depth: u32,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            disabled: Vec::new(),
            max_function_lines: 50,
            max_function_complexity: 15,
            max_params: 5,
            no_magic_numbers_min: 3,
            max_nested_depth: 4,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SignificantCodeConfig {
    /// Glob patterns (relative to root) of files considered "significant"
    /// production code. Issues in other files (tests, generated, etc.)
    /// are still reported but excluded from the "significant" count and
    /// quality-gate evaluation.
    pub patterns: Vec<String>,
    /// Common test file patterns. Issues here are excluded by default.
    pub test_patterns: Vec<String>,
    /// Generated-code patterns. Issues here are excluded.
    pub generated_patterns: Vec<String>,
}

impl Default for SignificantCodeConfig {
    fn default() -> Self {
        Self {
            patterns: Vec::new(),
            test_patterns: vec![
                "**/*.test.ts".into(),
                "**/*.test.tsx".into(),
                "**/*.test.js".into(),
                "**/*.spec.ts".into(),
                "**/*.spec.tsx".into(),
                "**/*.spec.js".into(),
                "**/__tests__/**".into(),
                "**/__mocks__/**".into(),
            ],
            generated_patterns: vec![
                "**/*.d.ts".into(),
                "**/*.generated.ts".into(),
                "**/generated/**".into(),
                "**/dist/**".into(),
                "**/build/**".into(),
            ],
        }
    }
}

impl Clone for SignificantCodeConfig {
    fn clone(&self) -> Self {
        Self {
            patterns: self.patterns.clone(),
            test_patterns: self.test_patterns.clone(),
            generated_patterns: self.generated_patterns.clone(),
        }
    }
}

impl SignificantCodeConfig {
    /// Returns true if a relative path is considered significant production
    /// code (i.e. NOT a test, NOT generated, and matches a significant
    /// pattern if any are configured).
    pub fn is_significant(&self, rel_path: &str) -> bool {
        // If any test pattern matches → not significant.
        for p in &self.test_patterns {
            if glob_match(p, rel_path) {
                return false;
            }
        }
        // Generated code → not significant.
        for p in &self.generated_patterns {
            if glob_match(p, rel_path) {
                return false;
            }
        }
        // If significant patterns are configured, file must match at least one.
        if !self.patterns.is_empty() {
            return self.patterns.iter().any(|p| glob_match(p, rel_path));
        }
        // Default: everything is significant.
        true
    }
}

/// Glob matcher that supports `*` and `**`. Used by `SignificantCodeConfig`.
/// Not a full glob implementation — just enough for common patterns.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    // Normalize separators
    let p = pattern.replace('\\', "/");
    let s = path.replace('\\', "/");
    glob_recurse(&p, &s)
}

fn glob_recurse(pattern: &str, s: &str) -> bool {
    if pattern.is_empty() {
        return s.is_empty();
    }
    if let Some(rest) = pattern.strip_prefix("**/") {
        // Try matching at every position in s.
        for i in 0..=s.len() {
            if glob_recurse(rest, &s[i..]) {
                return true;
            }
        }
        return false;
    }
    if let Some(c) = pattern.chars().next() {
        if c == '*' {
            // Match any sequence (no slash) until next literal char.
            let next = &pattern[1..];
            for i in 0..=s.len() {
                // If next is empty, '*' can match the rest (no / restriction).
                if next.is_empty() {
                    if !s[..i].contains('/') {
                        return true;
                    }
                } else if glob_recurse(next, &s[i..]) {
                    return true;
                }
            }
            return false;
        }
    }
    // Literal char.
    if s.is_empty() {
        return false;
    }
    if pattern.chars().next() == s.chars().next() {
        return glob_recurse(&pattern[1..], &s[1..]);
    }
    false
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
        Self {
            output: PathBuf::from("lens-report"),
            open_browser: false,
        }
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
        let cfg: Self =
            toml::from_str(&text).with_context(|| format!("parsing config: {}", path.display()))?;
        Ok(cfg)
    }

    /// Resolve the actual config path: explicit path > quality-gate.toml in scan root.
    pub fn resolve_path(explicit: Option<&Path>, scan_root: &Path) -> Option<PathBuf> {
        if let Some(p) = explicit {
            return Some(p.to_path_buf());
        }
        let candidate = scan_root.join(CONFIG_FILENAME);
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_spec() {
        assert!(glob_match("**/*.spec.ts", "copy.spec.ts"));
        assert!(glob_match("**/*.spec.ts", "src/foo/copy.spec.ts"));
        assert!(!glob_match("**/*.spec.ts", "src/foo/copy.ts"));
    }

    #[test]
    fn test_significant_code() {
        let cfg = SignificantCodeConfig::default();
        assert!(
            !cfg.is_significant("copy.spec.ts"),
            "spec file should not be significant"
        );
        assert!(
            !cfg.is_significant("src/foo.test.ts"),
            "test file should not be significant"
        );
        assert!(
            !cfg.is_significant("src/foo.d.ts"),
            ".d.ts should not be significant"
        );
        assert!(
            cfg.is_significant("src/service.ts"),
            "production code should be significant"
        );
        assert!(
            cfg.is_significant("src/utils.ts"),
            "production code should be significant"
        );
    }
}
