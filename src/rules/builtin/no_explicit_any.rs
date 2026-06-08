//! `no-explicit-any` — flags explicit `any` type annotations in TypeScript.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoExplicitAny;

impl Rule for NoExplicitAny {
    fn id(&self) -> &'static str {
        "no-explicit-any"
    }
    fn name(&self) -> &'static str {
        "No explicit `any`"
    }
    fn description(&self) -> &'static str {
        "Avoid using the `any` type. Use a specific type or `unknown` instead."
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[Language::TypeScript, Language::Tsx]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lang = match file.language {
            Some(L) if L == Language::TypeScript || L == Language::Tsx => L,
            _ => return issues,
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if is_any_type(node, source) {
                    let start = node.start_position();
                    issues.push(Issue {
                        rule_id: "no-explicit-any".into(),
                        severity: Severity::Major,
                        message: "Avoid the `any` type; use a specific type or `unknown`.".into(),
                        file: file.path.clone(),
                        start_line: start.row as u32 + 1,
                        end_line: start.row as u32 + 1,
                        start_column: start.column as u32,
                        end_column: (start.column + node_text_len(node, "any")) as u32,
                    });
                }
            });
        });
        issues
    }
}

fn is_any_type(node: Node, source: &str) -> bool {
    // tree-sitter-typescript uses "any" for the `any` keyword in type positions
    // (predefined_type → "any") and as a type_identifier in some cases.
    if node.kind() == "predefined_type" {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            return text == "any";
        }
    }
    if node.kind() == "type_identifier" {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            return text == "any";
        }
    }
    false
}

fn node_text_len(node: Node, _expected: &str) -> usize {
    node.end_position().column - node.start_position().column
}
