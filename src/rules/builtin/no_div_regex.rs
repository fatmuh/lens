//! No div regex.
//!
//! Disallow regex like division

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoDivRegex;

impl Rule for NoDivRegex {
    fn id(&self) -> &'static str {
        "no-div-regex"
    }
    fn name(&self) -> &'static str {
        "No div regex"
    }
    fn description(&self) -> &'static str {
        "Disallow regex like division"
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
        let _ = (file, source); // TODO: AST for no-div-regex
        issues
    }
}
