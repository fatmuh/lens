//! No duplicate case.
//!
//! Disallow duplicate case labels

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoDuplicateCase;

impl Rule for NoDuplicateCase {
    fn id(&self) -> &'static str {
        "no-duplicate-case"
    }
    fn name(&self) -> &'static str {
        "No duplicate case"
    }
    fn description(&self) -> &'static str {
        "Disallow duplicate case labels"
    }
    fn default_severity(&self) -> Severity {
        Severity::Critical
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
        let _ = (file, source); // TODO: AST for no-duplicate-case
        issues
    }
}
