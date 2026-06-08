//! No self-assignment.
//!
//! Disallow self-assignment

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoSelfAssign;

impl Rule for NoSelfAssign {
    fn id(&self) -> &'static str {
        "no-self-assign"
    }
    fn name(&self) -> &'static str {
        "No self-assignment"
    }
    fn description(&self) -> &'static str {
        "Disallow self-assignment"
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
        let _ = (file, source); // TODO: AST for no-self-assign
        issues
    }
}
