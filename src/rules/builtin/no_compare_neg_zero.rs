//! No comparison with -0.
//!
//! Disallow comparing against -0

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoCompareNegZero;

impl Rule for NoCompareNegZero {
    fn id(&self) -> &'static str {
        "no-compare-neg-zero"
    }
    fn name(&self) -> &'static str {
        "No comparison with -0"
    }
    fn description(&self) -> &'static str {
        "Disallow comparing against -0"
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
        if let Ok(re) = regex::Regex::new(r"===\s*-0\b|==\s*-0\b") {
            for cap in re.captures_iter(source) {
                let m = cap.get(0).unwrap();
                let line = source[..m.start()].lines().count() as u32 + 1;
                issues.push(Issue {
                    rule_id: "no-compare-neg-zero".to_string(),
                    severity: self.default_severity(),
                    message: "Do not compare against -0.".to_string(),
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
