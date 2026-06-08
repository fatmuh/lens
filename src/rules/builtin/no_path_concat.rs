//! No path concatenation.
//!
//! Disallow __dirname + concatenation

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoPathConcat;

impl Rule for NoPathConcat {
    fn id(&self) -> &'static str {
        "no-path-concat"
    }
    fn name(&self) -> &'static str {
        "No path concatenation"
    }
    fn description(&self) -> &'static str {
        "Disallow __dirname + concatenation"
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
        if let Ok(re) = regex::Regex::new(r"(__dirname|__filename)\s*\+") {
            for cap in re.captures_iter(source) {
                let m = cap.get(0).unwrap();
                let line = source[..m.start()].lines().count() as u32 + 1;
                issues.push(Issue {
                    rule_id: "no-path-concat".to_string(),
                    severity: self.default_severity(),
                    message: "Use path.join().".to_string(),
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
