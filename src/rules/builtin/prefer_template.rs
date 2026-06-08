//! `prefer-template` — flags `+` between strings/identifiers where a
//! template literal would be clearer.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct PreferTemplate;

impl Rule for PreferTemplate {
    fn id(&self) -> &'static str {
        "prefer-template"
    }
    fn name(&self) -> &'static str {
        "Prefer template literals"
    }
    fn description(&self) -> &'static str {
        "Use template literals (`` `Hello ${name}` ``) instead of string concatenation."
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
                let op_text = match op.utf8_text(source.as_bytes()) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                if op_text != "+" {
                    return;
                }
                let left = node.child_by_field_name("left");
                let right = node.child_by_field_name("right");
                let (Some(l), Some(r)) = (left, right) else {
                    return;
                };
                if is_stringish(l)
                    && (is_stringish(r)
                        || matches!(
                            r.kind(),
                            "identifier" | "member_expression" | "call_expression"
                        ))
                {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "prefer-template".into(),
                        severity: Severity::Minor,
                        message: "Use a template literal instead of string concatenation.".into(),
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

fn is_stringish(node: Node) -> bool {
    matches!(node.kind(), "string" | "template_string")
}
