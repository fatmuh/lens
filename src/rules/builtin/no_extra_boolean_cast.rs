//! `no-extra-boolean-cast` — flags `!!x` or `Boolean(x)` when `x` is already boolean.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoExtraBooleanCast;

impl Rule for NoExtraBooleanCast {
    fn id(&self) -> &'static str { "no-extra-boolean-cast" }
    fn name(&self) -> &'static str { "No extra boolean cast" }
    fn description(&self) -> &'static str {
        "Don't use `!!x` or `Boolean(x)` on a value that's already boolean."
    }
    fn default_severity(&self) -> Severity { Severity::Minor }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                // `!!x` → unary_expression with `!` on a unary_expression with `!`
                if node.kind() == "unary_expression" {
                    if let Ok(text) = node.utf8_text(source.as_bytes()) {
                        if text.starts_with("!!") {
                            let start = node.start_position();
                            let end = node.end_position();
                            issues.push(Issue {
                                rule_id: "no-extra-boolean-cast".into(),
                                severity: Severity::Minor,
                                message: "Redundant `!!`; the inner expression is already boolean.".into(),
                                file: file.path.clone(),
                                start_line: start.row as u32 + 1,
                                end_line: end.row as u32 + 1,
                                start_column: start.column as u32,
                                end_column: end.column as u32,
                            });
                            return;
                        }
                    }
                }
                // `Boolean(x)` where x is a comparison or already boolean
                if node.kind() == "call_expression" {
                    let Some(func) = node.child_by_field_name("function") else { return; };
                    if func.kind() == "identifier" {
                        if let Ok(name) = func.utf8_text(source.as_bytes()) {
                            if name != "Boolean" { return; }
                            // Get the argument
                            let mut cursor = node.walk();
                            for arg in node.children(&mut cursor) {
                                if arg.kind() == "arguments" {
                                    let mut ac = arg.walk();
                                    for a in arg.children(&mut ac) {
                                        if is_already_boolean(a) {
                                            let start = node.start_position();
                                            let end = node.end_position();
                                            issues.push(Issue {
                                                rule_id: "no-extra-boolean-cast".into(),
                                                severity: Severity::Minor,
                                                message: "Redundant `Boolean()` call; the argument is already boolean.".into(),
                                                file: file.path.clone(),
                                                start_line: start.row as u32 + 1,
                                                end_line: end.row as u32 + 1,
                                                start_column: start.column as u32,
                                                end_column: end.column as u32,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
        });
        issues
    }
}

fn is_already_boolean(node: Node) -> bool {
    matches!(node.kind(),
        "binary_expression"           // x === y
        | "unary_expression"         // !x
        | "comparison_expression"    // x < y
        | "true" | "false"
    )
}
