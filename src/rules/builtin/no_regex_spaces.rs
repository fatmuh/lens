//! No multiple spaces in regex.
//!
//! Disallow spaces in regex

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoRegexSpaces;

impl Rule for NoRegexSpaces {
    fn id(&self) -> &'static str {
        "no-regex-spaces"
    }
    fn name(&self) -> &'static str {
        "No multiple spaces in regex"
    }
    fn description(&self) -> &'static str {
        "Disallow spaces in regex"
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
        let _ = (file, source); // TODO: AST for no-regex-spaces
        issues
    }
}
