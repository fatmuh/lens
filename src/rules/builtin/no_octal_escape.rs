//! No octal escape.
//!
//! Disallow octal escapes

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoOctalEscape;

impl Rule for NoOctalEscape {
    fn id(&self) -> &'static str {
        "no-octal-escape"
    }
    fn name(&self) -> &'static str {
        "No octal escape"
    }
    fn description(&self) -> &'static str {
        "Disallow octal escapes"
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[
            Language::TypeScript,
            Language::Tsx,
            Language::JavaScript,
            Language::Jsx,
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let _ = (file, source); // TODO: AST for no-octal-escape
        issues
    }
}
