//! No function reassignment.
//!
//! Disallow function reassignment

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoFuncAssign;

impl Rule for NoFuncAssign {
    fn id(&self) -> &'static str {
        "no-func-assign"
    }
    fn name(&self) -> &'static str {
        "No function reassignment"
    }
    fn description(&self) -> &'static str {
        "Disallow function reassignment"
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
        let _ = (file, source); // TODO: AST for no-func-assign
        issues
    }
}
