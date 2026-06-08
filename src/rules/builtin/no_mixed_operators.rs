//! No mixed operators.
//!
//! Disallow mixed operators

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoMixedOperators;

impl Rule for NoMixedOperators {
    fn id(&self) -> &'static str {
        "no-mixed-operators"
    }
    fn name(&self) -> &'static str {
        "No mixed operators"
    }
    fn description(&self) -> &'static str {
        "Disallow mixed operators"
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
        let _ = (file, source); // TODO: AST for no-mixed-operators
        issues
    }
}
