//! No catch reassignment.
//!
//! Disallow catch reassignment

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoExAssign;

impl Rule for NoExAssign {
    fn id(&self) -> &'static str {
        "no-ex-assign"
    }
    fn name(&self) -> &'static str {
        "No catch reassignment"
    }
    fn description(&self) -> &'static str {
        "Disallow catch reassignment"
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
        let _ = (file, source); // TODO: AST for no-ex-assign
        issues
    }
}
