//! No useless escape.
//!
//! Disallow useless escapes

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUselessEscape;

impl Rule for NoUselessEscape {
    fn id(&self) -> &'static str {
        "no-useless-escape"
    }
    fn name(&self) -> &'static str {
        "No useless escape"
    }
    fn description(&self) -> &'static str {
        "Disallow useless escapes"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
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
        let _ = (file, source); // TODO: AST for no-useless-escape
        issues
    }
}
