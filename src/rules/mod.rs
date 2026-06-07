//! Rule engine (Phase 2).
//!
//! Defines the core abstractions:
//! - [`Severity`] — issue severity (blocker → info).
//! - [`Issue`] — a single problem found in a file.
//! - [`Rule`] — a trait rules implement to check a file.
//! - [`RuleRegistry`] — collection of built-in rules, used by the scanner.
//!
//! Built-in rules live in [`builtin`]. The default registry returns all of
//! them, with rules for the project language enabled automatically.

pub mod builtin;

#[cfg(test)]
mod rule_tests;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::analyzer::FileAnalysis;
use crate::scanner::language::Language;

/// Issue severity. Ordered from most to least urgent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Bug, security issue, or correctness problem. Must be fixed.
    Blocker,
    /// Likely bug or significant code smell. Should be fixed.
    Critical,
    /// Substantial code smell or maintainability issue.
    Major,
    /// Minor issue, suggestion, or style.
    Minor,
    /// Informational hint.
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Blocker => "blocker",
            Severity::Critical => "critical",
            Severity::Major => "major",
            Severity::Minor => "minor",
            Severity::Info => "info",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "blocker" => Some(Severity::Blocker),
            "critical" => Some(Severity::Critical),
            "major" => Some(Severity::Major),
            "minor" => Some(Severity::Minor),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }
}

/// A single problem found in a file by a rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    /// Stable rule identifier, e.g. `"no-explicit-any"`.
    pub rule_id: String,
    pub severity: Severity,
    /// One-line message describing the issue.
    pub message: String,
    pub file: PathBuf,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
}

/// A rule that inspects a file and reports any [`Issue`]s it finds.
///
/// Rules are stateless and `Send + Sync` so the scanner can run them
/// in parallel across files.
pub trait Rule: Send + Sync {
    /// Stable, kebab-case identifier (e.g. `"no-explicit-any"`).
    fn id(&self) -> &'static str;
    /// Short human-readable name.
    fn name(&self) -> &'static str;
    /// Longer description shown in `lens rules`.
    fn description(&self) -> &'static str;
    /// Default severity. Users can override per-rule in config.
    fn default_severity(&self) -> Severity;
    /// Languages this rule applies to. Empty = all languages.
    fn languages(&self) -> &[Language] {
        &[]
    }
    /// Inspect `source` and return any issues. `file` provides context
    /// (path, metrics, tokens).
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue>;
}

/// Per-rule severity override from `quality-gate.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RuleOverride {
    /// Override the default severity.
    pub severity: Option<Severity>,
}

/// Registry of all available rules. Built from a default set of built-in
/// rules; users can disable individual rules via configuration.
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
}

impl std::fmt::Debug for RuleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleRegistry")
            .field("count", &self.rules.len())
            .field("ids", &self.rules.iter().map(|r| r.id()).collect::<Vec<_>>())
            .finish()
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}

impl RuleRegistry {
    /// Build the default registry with all built-in rules enabled.
    pub fn default_registry() -> Self {
        Self { rules: builtin::all_rules() }
    }

    /// All rules in the registry.
    pub fn rules(&self) -> &[Box<dyn Rule>] {
        &self.rules
    }

    /// Run every rule on a file and return all reported issues.
    pub fn run(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for rule in &self.rules {
            issues.extend(rule.check(file, source));
        }
        issues
    }

    /// Build a registry with specific rule IDs disabled.
    pub fn with_disabled(disabled: &[String]) -> Self {
        let mut reg = Self::default_registry();
        reg.rules.retain(|r| !disabled.iter().any(|d| d == r.id()));
        reg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn severity_ordering() {
        assert!(Severity::Blocker < Severity::Critical);
        assert!(Severity::Critical < Severity::Major);
        assert!(Severity::Major < Severity::Minor);
        assert!(Severity::Minor < Severity::Info);
    }

    #[test]
    fn severity_round_trip() {
        for s in [Severity::Blocker, Severity::Critical, Severity::Major, Severity::Minor, Severity::Info] {
            assert_eq!(Severity::from_str(s.as_str()), Some(s));
        }
        assert_eq!(Severity::from_str("unknown"), None);
    }

    #[test]
    fn registry_default_has_rules() {
        let reg = RuleRegistry::default_registry();
        assert!(!reg.rules().is_empty(), "default registry should have rules");
    }

    #[test]
    fn registry_with_disabled() {
        let reg = RuleRegistry::with_disabled(&["no-console".to_string()]);
        assert!(!reg.rules().iter().any(|r| r.id() == "no-console"));
        assert!(reg.rules().iter().any(|r| r.id() != "no-console"));
    }

    #[test]
    fn issue_serialize() {
        let issue = Issue {
            rule_id: "no-explicit-any".into(),
            severity: Severity::Major,
            message: "Avoid `any`".into(),
            file: PathBuf::from("src/foo.ts"),
            start_line: 1,
            end_line: 1,
            start_column: 5,
            end_column: 8,
        };
        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("no-explicit-any"));
        assert!(json.contains("major"));
    }
}
