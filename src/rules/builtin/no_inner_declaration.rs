//! No inner declarations.
//!
//! Disallow declarations in blocks

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoInnerDeclaration;

impl Rule for NoInnerDeclaration {
    fn id(&self) -> &'static str {
        "no-inner-declaration"
    }
    fn name(&self) -> &'static str {
        "No inner declarations"
    }
    fn description(&self) -> &'static str {
        "Disallow declarations in blocks"
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
        let _ = (file, source); // TODO: AST for no-inner-declaration
        issues
    }
}
