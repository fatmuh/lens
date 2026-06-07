//! `max-function-complexity` — flags functions whose cyclomatic complexity
//! exceeds a threshold (default 10).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct MaxFunctionComplexity;

const DEFAULT_THRESHOLD: u32 = 10;

impl Rule for MaxFunctionComplexity {
    fn id(&self) -> &'static str { "max-function-complexity" }
    fn name(&self) -> &'static str { "Function too complex" }
    fn description(&self) -> &'static str {
        "Cyclomatic complexity > 10 makes functions hard to test. Refactor into smaller pieces."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, _source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        if !matches!(file.language, Some(Language::TypeScript) | Some(Language::Tsx)) {
            return issues;
        }
        let Some(metrics) = &file.metrics else { return issues };
        for func in &metrics.functions {
            if func.complexity > DEFAULT_THRESHOLD {
                issues.push(Issue {
                    rule_id: "max-function-complexity".into(),
                    severity: Severity::Major,
                    message: format!(
                        "Function `{}` has cyclomatic complexity {} (max {}).",
                        func.name, func.complexity, DEFAULT_THRESHOLD
                    ),
                    file: file.path.clone(),
                    start_line: func.start_line,
                    end_line: func.end_line,
                    start_column: 0,
                    end_column: 0,
                });
            }
        }
        issues
    }
}
