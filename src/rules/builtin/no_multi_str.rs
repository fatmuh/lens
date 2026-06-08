//! No multiline strings.
//!
//! Disallow backslash multiline

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoMultiStr;

impl Rule for NoMultiStr {
    fn id(&self) -> &'static str {
        "no-multi-str"
    }
    fn name(&self) -> &'static str {
        "No multiline strings"
    }
    fn description(&self) -> &'static str {
        "Disallow backslash multiline"
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
        let _ = (file, source); // TODO: AST for no-multi-str
        issues
    }
}
