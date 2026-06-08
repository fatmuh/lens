//! `no-nested-ternary` — flags `a ? b : c ? d : e` (ternary inside ternary).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoNestedTernary;

impl Rule for NoNestedTernary {
    fn id(&self) -> &'static str {
        "no-nested-ternary"
    }
    fn name(&self) -> &'static str {
        "No nested ternary"
    }
    fn description(&self) -> &'static str {
        "Avoid nested ternaries — they are hard to read. Use `if`/`else`."
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
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "ternary_expression" {
                    return;
                }
                // Check if any descendant is also a ternary.
                let mut nested = false;
                crate::analyzer::parser::visit_descendants(node, |n| {
                    if n == node {
                        return;
                    }
                    if n.kind() == "ternary_expression" {
                        nested = true;
                    }
                });
                if nested {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-nested-ternary".into(),
                        severity: Severity::Minor,
                        message: "Avoid nested ternaries; use `if`/`else` for clarity.".into(),
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
