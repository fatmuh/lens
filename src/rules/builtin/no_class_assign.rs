//! No class reassignment.
//!
//! Disallow class reassignment

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoClassAssign;

impl Rule for NoClassAssign {
    fn id(&self) -> &'static str {
        "no-class-assign"
    }
    fn name(&self) -> &'static str {
        "No class reassignment"
    }
    fn description(&self) -> &'static str {
        "Disallow class reassignment"
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
        let _ = (file, source); // TODO: AST for no-class-assign
        issues
    }
}
