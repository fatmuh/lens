//! No empty pattern.
//!
//! Disallow empty destructuring

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoEmptyPattern;

impl Rule for NoEmptyPattern {
    fn id(&self) -> &'static str {
        "no-empty-pattern"
    }
    fn name(&self) -> &'static str {
        "No empty pattern"
    }
    fn description(&self) -> &'static str {
        "Disallow empty destructuring"
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
            Language::Dart,
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        if let Ok(re) = regex::Regex::new(r"\{\s*\}\s*=") {
            for cap in re.captures_iter(source) {
                let m = cap.get(0).unwrap();
                let line = source[..m.start()].lines().count() as u32 + 1;
                issues.push(Issue {
                    rule_id: "no-empty-pattern".to_string(),
                    severity: self.default_severity(),
                    message: "Empty destructuring pattern.".to_string(),
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
