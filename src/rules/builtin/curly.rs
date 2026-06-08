//! Require braces.
//!
//! Require curly braces

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct Curly;

impl Rule for Curly {
    fn id(&self) -> &'static str {
        "curly"
    }
    fn name(&self) -> &'static str {
        "Require braces"
    }
    fn description(&self) -> &'static str {
        "Require curly braces"
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
        let _ = (file, source); // TODO: AST for curly
        issues
    }
}
