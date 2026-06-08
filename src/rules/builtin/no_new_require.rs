//! No new require.
//!
//! Disallow new require

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoNewRequire;

impl Rule for NoNewRequire {
    fn id(&self) -> &'static str {
        "no-new-require"
    }
    fn name(&self) -> &'static str {
        "No new require"
    }
    fn description(&self) -> &'static str {
        "Disallow new require"
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
        if let Ok(re) = regex::Regex::new(r"\bnew\s+require\s*\(") {
            for cap in re.captures_iter(source) {
                let m = cap.get(0).unwrap();
                let line = source[..m.start()].lines().count() as u32 + 1;
                issues.push(Issue {
                    rule_id: "no-new-require".to_string(),
                    severity: self.default_severity(),
                    message: "Unexpected new require.".to_string(),
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
