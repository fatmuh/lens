//! Use shorthand assignment.
//!
//! Require compound operators

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct OperatorAssignment;

impl Rule for OperatorAssignment {
    fn id(&self) -> &'static str {
        "operator-assignment"
    }
    fn name(&self) -> &'static str {
        "Use shorthand assignment"
    }
    fn description(&self) -> &'static str {
        "Require compound operators"
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
            Language::Dart,
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let _ = (file, source); // TODO: AST for operator-assignment
        issues
    }
}
