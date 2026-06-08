//! No new without assignment.
//!
//! Disallow new without store

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoNew;

impl Rule for NoNew {
    fn id(&self) -> &'static str {
        "no-new"
    }
    fn name(&self) -> &'static str {
        "No new without assignment"
    }
    fn description(&self) -> &'static str {
        "Disallow new without store"
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
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
        let _ = (file, source); // TODO: AST for no-new
        issues
    }
}
