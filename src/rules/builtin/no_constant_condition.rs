//! No constant conditions.
//!
//! Disallow constant conditions

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoConstantCondition;

impl Rule for NoConstantCondition {
    fn id(&self) -> &'static str {
        "no-constant-condition"
    }
    fn name(&self) -> &'static str {
        "No constant conditions"
    }
    fn description(&self) -> &'static str {
        "Disallow constant conditions"
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
        if let Ok(re) = regex::Regex::new(r"\b(if|while)\s*\(\s*(true|false|null|undefined)\s*\)") {
            for cap in re.captures_iter(source) {
                let m = cap.get(0).unwrap();
                let line = source[..m.start()].lines().count() as u32 + 1;
                issues.push(Issue {
                    rule_id: "no-constant-condition".to_string(),
                    severity: self.default_severity(),
                    message: "Constant condition.".to_string(),
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
