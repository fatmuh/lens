//! `no-useless-concat` — flags `'a' + 'b'` (concatenation of two literals).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUselessConcat;

impl Rule for NoUselessConcat {
    fn id(&self) -> &'static str {
        "no-useless-concat"
    }
    fn name(&self) -> &'static str {
        "No useless string concatenation"
    }
    fn description(&self) -> &'static str {
        "Concatenating two string literals (`'a' + 'b'`) is unnecessary; use a single literal."
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
            Language::Dart,
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "binary_expression" {
                    return;
                }
                let Some(op) = node.child_by_field_name("operator") else {
                    return;
                };
                if op
                    .utf8_text(source.as_bytes())
                    .map(|t| t != "+")
                    .unwrap_or(true)
                {
                    return;
                }
                let Some(left) = node.child_by_field_name("left") else {
                    return;
                };
                let Some(right) = node.child_by_field_name("right") else {
                    return;
                };
                if matches!(left.kind(), "string" | "template_string")
                    && matches!(right.kind(), "string" | "template_string")
                {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-useless-concat".into(),
                        severity: Severity::Minor,
                        message: "Concatenating two literals is unnecessary; use a single literal."
                            .into(),
                        file: file.path.clone(),
                        start_line: start.row as u32 + 1,
                        end_line: end.row as u32 + 1,
                        start_column: start.column as u32,
                        end_column: end.column as u32,
                    });
                }
            });
        });
        issues
    }
}
