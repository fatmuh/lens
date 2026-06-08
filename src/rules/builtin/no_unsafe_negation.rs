//! No unsafe negation.
//!
//! Disallow unsafe negation of in/instanceof

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUnsafeNegation;

impl Rule for NoUnsafeNegation {
    fn id(&self) -> &'static str {
        "no-unsafe-negation"
    }
    fn name(&self) -> &'static str {
        "No unsafe negation"
    }
    fn description(&self) -> &'static str {
        "Disallow unsafe negation of in/instanceof"
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
        if let Ok(re) = regex::Regex::new(r"!\s*\w+\s+in\b|!\s*\w+\s+instanceof\b") {
            for cap in re.captures_iter(source) {
                let m = cap.get(0).unwrap();
                let line = source[..m.start()].lines().count() as u32 + 1;
                issues.push(Issue {
                    rule_id: "no-unsafe-negation".to_string(),
                    severity: self.default_severity(),
                    message: "Unexpected negation.".to_string(),
                    file: file.path.clone(),
                    start_line: line,
                    end_line: line,
                    start_column: 0,
                    end_column: 0,
                });
            }
        }
        issues
    }
}
