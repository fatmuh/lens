//! One variable per declaration.
//!
//! One var per declaration

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct OneVar;

impl Rule for OneVar {
    fn id(&self) -> &'static str {
        "one-var"
    }
    fn name(&self) -> &'static str {
        "One variable per declaration"
    }
    fn description(&self) -> &'static str {
        "One var per declaration"
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
        let _ = (file, source); // TODO: AST for one-var
        issues
    }
}
