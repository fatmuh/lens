//! No delete on variables.
//!
//! Disallow delete on variables

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoDeleteVar;

impl Rule for NoDeleteVar {
    fn id(&self) -> &'static str {
        "no-delete-var"
    }
    fn name(&self) -> &'static str {
        "No delete on variables"
    }
    fn description(&self) -> &'static str {
        "Disallow delete on variables"
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
        if let Ok(re) = regex::Regex::new(r"\bdelete\s+[a-zA-Z_$]") {
            for cap in re.captures_iter(source) {
                let m = cap.get(0).unwrap();
                let line = source[..m.start()].lines().count() as u32 + 1;
                issues.push(Issue {
                    rule_id: "no-delete-var".to_string(),
                    severity: self.default_severity(),
                    message: "Cannot delete variables.".to_string(),
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
