//! No template curly in string.
//!
//! Disallow template syntax in strings

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoTemplateCurlyInString;

impl Rule for NoTemplateCurlyInString {
    fn id(&self) -> &'static str {
        "no-template-curly-in-string"
    }
    fn name(&self) -> &'static str {
        "No template curly in string"
    }
    fn description(&self) -> &'static str {
        "Disallow template syntax in strings"
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
        let _ = (file, source); // TODO: AST for no-template-curly-in-string
        issues
    }
}
