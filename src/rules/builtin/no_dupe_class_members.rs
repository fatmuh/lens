//! No duplicate class members.
//!
//! Disallow duplicate class members

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoDupeClassMembers;

impl Rule for NoDupeClassMembers {
    fn id(&self) -> &'static str {
        "no-dupe-class-members"
    }
    fn name(&self) -> &'static str {
        "No duplicate class members"
    }
    fn description(&self) -> &'static str {
        "Disallow duplicate class members"
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
            Language::Dart,
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let _ = (file, source); // TODO: AST for no-dupe-class-members
        issues
    }
}
