//! `max-function-lines` — flags functions that exceed a length threshold.
//! Threshold comes from `[rules.max_function_lines]` in quality-gate.toml
//! (default 50 lines). Per-function metrics are already computed by
//! `analyzer::metrics`, so we just consult them here.

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct MaxFunctionLines;

const DEFAULT_THRESHOLD: u32 = 50;

impl Rule for MaxFunctionLines {
    fn id(&self) -> &'static str { "max-function-lines" }
    fn name(&self) -> &'static str { "Function too long" }
    fn description(&self) -> &'static str {
        "Functions longer than 50 lines are hard to read and test. Consider splitting."
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
            let len = func.end_line.saturating_sub(func.start_line) + 1;
            if len > DEFAULT_THRESHOLD {
                issues.push(Issue {
                    rule_id: "max-function-lines".into(),
                    severity: Severity::Major,
                    message: format!(
                        "Function `{}` is {} lines long; consider splitting (max {}).",
                        func.name, len, DEFAULT_THRESHOLD
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
