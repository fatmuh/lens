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
    /// Exclude test files from analysis (files matching test patterns).
    /// When true, test files are skipped entirely (no issues, no metrics).
    /// When false (default), test files are scanned but excluded from
    /// duplication and quality gate (matches SonarQube behavior).
    pub exclude_tests: bool,
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
            exclude_tests: false,
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
    /// Algorithm to use: "sonar" (default, SonarQube-compatible) or "token" (sensitive).
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
            mode: "sonar".to_string(),
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
    /// User-defined custom rules (regex-based).
    #[serde(default)]
    pub custom: Vec<CustomRuleConfig>,
}

/// A user-defined rule loaded from `quality-gate.toml`.
///
/// Custom rules are regex-based: Lens scans each line of source files
/// matching the configured `languages` and flags matches as issues.
///
/// # Example
///
/// ```toml
/// [[rules.custom]]
/// id = "no-hardcoded-api-keys"
/// name = "No hardcoded API keys"
/// description = "Detect hardcoded API keys in source code"
/// severity = "blocker"
/// languages = ["typescript", "javascript", "dart"]
/// pattern = "(?i)(api[_-]?key|secret[_-]?key)\\s*[:=]\\s*['\\"][a-zA-Z0-9]{20,}['\\"]"
/// message = "Hardcoded API key detected. Use environment variables instead."
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRuleConfig {
    /// Unique rule ID (kebab-case).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Longer description.
    pub description: String,
    /// Severity: `"blocker"`, `"critical"`, `"major"`, `"minor"`, or `"info"`.
    #[serde(default = "default_custom_severity")]
    pub severity: String,
    /// Languages this rule applies to. Empty = all languages.
    #[serde(default)]
    pub languages: Vec<String>,
    /// Regex pattern to search for in each line.
    pub pattern: String,
    /// Message shown for each match.
    #[serde(default)]
    pub message: String,
}

fn default_custom_severity() -> String {
    "major".to_string()
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
            custom: Vec::new(),
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
    /// Also reads `sonar-project.properties` from the config's parent directory
    /// and merges exclusions/test patterns into the scan config.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };

        if !path.exists() {
            anyhow::bail!("config file not found: {}", path.display());
        }

        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config: {}", path.display()))?;
        let mut cfg: Self =
            toml::from_str(&text).with_context(|| format!("parsing config: {}", path.display()))?;

        // Try to read sonar-project.properties from the same directory
        if let Some(parent) = path.parent() {
            let sonar_path = parent.join("sonar-project.properties");
            if sonar_path.exists() {
                merge_sonar_properties(&mut cfg, &sonar_path);
            }
        }

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
            // No quality-gate.toml — create a default config and merge
            // sonar-project.properties if it exists
            None
        }
    }

    /// Load config for a scan root, falling back to defaults if no
    /// quality-gate.toml exists. Still reads sonar-project.properties.
    pub fn load_for_root(explicit: Option<&Path>, scan_root: &Path) -> Result<Self> {
        if let Some(p) = explicit {
            return Self::load(Some(p));
        }

        let candidate = scan_root.join(CONFIG_FILENAME);
        if candidate.exists() {
            Self::load(Some(&candidate))
        } else {
            // No quality-gate.toml — use defaults but still read sonar
            let mut cfg = Self::default();
            let sonar_path = scan_root.join("sonar-project.properties");
            if sonar_path.exists() {
                merge_sonar_properties(&mut cfg, &sonar_path);
            }
            Ok(cfg)
        }
    }
}

/// Parse `sonar-project.properties` and merge exclusions, sources, and test
/// patterns into the Lens config. This allows Lens to work out-of-the-box
/// with projects that already have SonarQube configuration.
fn merge_sonar_properties(cfg: &mut Config, path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };

    tracing::info!("reading sonar-project.properties: {}", path.display());

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse key=value
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match key {
            // sonar.exclusions → scan.exclude
            "sonar.exclusions" => {
                let patterns = parse_sonar_globs(value);
                if !patterns.is_empty() {
                    tracing::info!("  sonar.exclusions → {} pattern(s)", patterns.len());
                    cfg.scan.exclude.extend(patterns);
                }
            }

            // sonar.sources → scan.include
            "sonar.sources" => {
                let dirs = parse_sonar_globs(value);
                if !dirs.is_empty() && cfg.scan.include.is_empty() {
                    // Convert source dirs to glob patterns
                    let globs: Vec<String> = dirs
                        .iter()
                        .flat_map(|d| {
                            // sonar.sources=src → include src/**
                            if d.contains('*') {
                                vec![d.clone()]
                            } else {
                                vec![format!("{}/**", d.trim_end_matches('/'))]
                            }
                        })
                        .collect();
                    tracing::info!("  sonar.sources → {} pattern(s)", globs.len());
                    cfg.scan.include = globs;
                }
            }

            // sonar.tests → mark test directories
            "sonar.tests" => {
                let dirs = parse_sonar_globs(value);
                for d in &dirs {
                    let pattern = if d.contains('*') {
                        d.clone()
                    } else {
                        format!("{}/**", d.trim_end_matches('/'))
                    };
                    // Add to test_patterns for quality gate exclusion
                    cfg.significant_code.test_patterns.push(pattern);
                }
                if !dirs.is_empty() {
                    tracing::info!("  sonar.tests → {} test dir(s)", dirs.len());
                }
            }

            // sonar.test.inclusions → test file patterns
            "sonar.test.inclusions" => {
                let patterns = parse_sonar_globs(value);
                for p in &patterns {
                    cfg.significant_code.test_patterns.push(p.clone());
                    // Also exclude test files from duplication
                }
                if !patterns.is_empty() {
                    tracing::info!(
                        "  sonar.test.inclusions → {} test pattern(s)",
                        patterns.len()
                    );
                }
            }

            // sonar.test.exclusions → exclude test files from analysis
            "sonar.test.exclusions" => {
                let patterns = parse_sonar_globs(value);
                if !patterns.is_empty() {
                    cfg.scan.exclude.extend(patterns);
                }
            }

            // sonar.coverage.reportPaths → coverage report paths
            "sonar.coverage.reportPaths" | "sonar.javascript.lcov.reportPaths" => {
                let paths = parse_sonar_globs(value);
                if !paths.is_empty()
                    && cfg.coverage.report_paths.len() == 1
                    && cfg.coverage.report_paths[0] == "coverage/lcov.info"
                {
                    cfg.coverage.report_paths = paths;
                    tracing::info!(
                        "  sonar coverage → {} path(s)",
                        cfg.coverage.report_paths.len()
                    );
                }
            }

            _ => {}
        }
    }
}

/// Parse a comma-separated list of SonarQube glob patterns.
/// Handles patterns like `**/*.spec.ts,**/node_modules/**`
fn parse_sonar_globs(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}
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
