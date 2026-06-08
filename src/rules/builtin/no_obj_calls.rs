//! No global object calls.
//!
//! Disallow calling Math/JSON/Reflect

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoObjCalls;

impl Rule for NoObjCalls {
    fn id(&self) -> &'static str {
        "no-obj-calls"
    }
    fn name(&self) -> &'static str {
        "No global object calls"
    }
    fn description(&self) -> &'static str {
        "Disallow calling Math/JSON/Reflect"
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
        if let Ok(re) = regex::Regex::new(r"\b(Math|JSON|Reflect|Intl)\s*\(") {
            for cap in re.captures_iter(source) {
                let m = cap.get(0).unwrap();
                let line = source[..m.start()].lines().count() as u32 + 1;
                issues.push(Issue {
                    rule_id: "no-obj-calls".to_string(),
                    severity: self.default_severity(),
                    message: "Not callable.".to_string(),
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
