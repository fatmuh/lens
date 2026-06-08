//! No label same as variable.
//!
//! Disallow label=variable

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoLabelVar;

impl Rule for NoLabelVar {
    fn id(&self) -> &'static str {
        "no-label-var"
    }
    fn name(&self) -> &'static str {
        "No label same as variable"
    }
    fn description(&self) -> &'static str {
        "Disallow label=variable"
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
        let _ = (file, source); // TODO: AST for no-label-var
        issues
    }
}
