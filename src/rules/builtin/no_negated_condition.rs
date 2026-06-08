//! `no-negated-condition` — flags `if (!a) ...; else ...` where the
//! negation could be inlined to the else branch.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoNegatedCondition;

impl Rule for NoNegatedCondition {
    fn id(&self) -> &'static str {
        "no-negated-condition"
    }
    fn name(&self) -> &'static str {
        "No negated conditions"
    }
    fn description(&self) -> &'static str {
        "Avoid negating the condition in `if (!x) ...; else ...` — flip the branches instead."
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
                if node.kind() != "if_statement" {
                    return;
                }
                let Some(cond) = node.child_by_field_name("condition") else {
                    return;
                };
                // Condition may be wrapped in a parenthesized_expression.
                let cond_text = unwrap_paren_text(cond, source);
                let is_negated = cond_text.starts_with('!');
                if !is_negated {
                    return;
                }
                // Must have an else clause.
                let has_else = node
                    .children(&mut node.walk())
                    .any(|c| c.kind() == "else_clause");
                if has_else {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-negated-condition".into(),
                        severity: Severity::Minor,
                        message: "Negate the condition and swap the branches to remove `!`.".into(),
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

/// If the node is a `parenthesized_expression`, return its inner text;
/// otherwise return the node's text. Strips wrapping parens.
fn unwrap_paren_text(node: Node, source: &str) -> String {
    let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
    if node.kind() == "parenthesized_expression" {
        text.trim_start_matches('(')
            .trim_end_matches(')')
            .to_string()
    } else {
        text
    }
}
