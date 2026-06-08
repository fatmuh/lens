//! No implicit globals.
//!
//! Disallow undeclared vars

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoImplicitGlobals;

impl Rule for NoImplicitGlobals {
    fn id(&self) -> &'static str {
        "no-implicit-globals"
    }
    fn name(&self) -> &'static str {
        "No implicit globals"
    }
    fn description(&self) -> &'static str {
        "Disallow undeclared vars"
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
        let _ = (file, source); // TODO: AST for no-implicit-globals
        issues
    }
}
