//! No Array constructor.
//!
//! Disallow Array constructor

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoArrayConstructor;

impl Rule for NoArrayConstructor {
    fn id(&self) -> &'static str {
        "no-array-constructor"
    }
    fn name(&self) -> &'static str {
        "No Array constructor"
    }
    fn description(&self) -> &'static str {
        "Disallow Array constructor"
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
        if let Ok(re) = regex::Regex::new(r"\bnew\s+Array\s*\(") {
            for cap in re.captures_iter(source) {
                let m = cap.get(0).unwrap();
                let line = source[..m.start()].lines().count() as u32 + 1;
                issues.push(Issue {
                    rule_id: "no-array-constructor".to_string(),
                    severity: self.default_severity(),
                    message: "Use [] instead.".to_string(),
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
