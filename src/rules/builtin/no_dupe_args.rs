//! No duplicate args.
//!
//! Disallow duplicate args

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoDupeArgs;

impl Rule for NoDupeArgs {
    fn id(&self) -> &'static str {
        "no-dupe-args"
    }
    fn name(&self) -> &'static str {
        "No duplicate args"
    }
    fn description(&self) -> &'static str {
        "Disallow duplicate args"
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
        let _ = (file, source); // TODO: AST for no-dupe-args
        issues
    }
}
