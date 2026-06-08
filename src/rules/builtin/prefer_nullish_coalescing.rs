//! `prefer-nullish-coalescing` — flags `x || y` when both sides might be
//! nullish; suggests `x ?? y` instead (TypeScript only — `||` also coerces
//! `0`, `""`, `false`).

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct PreferNullishCoalescing;

impl Rule for PreferNullishCoalescing {
    fn id(&self) -> &'static str {
        "prefer-nullish-coalescing"
    }
    fn name(&self) -> &'static str {
        "Prefer `??` over `||`"
    }
    fn description(&self) -> &'static str {
        "Use `??` instead of `||` for nullish checks. `||` also catches `0` and `''`."
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::TypeScript, Language::Tsx]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        if !matches!(lang, Language::TypeScript | Language::Tsx) {
            return issues;
        }
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "binary_expression" {
                    return;
                }
                let Some(op) = node.child_by_field_name("operator") else {
                    return;
                };
                let Ok(op_text) = op.utf8_text(source.as_bytes()) else {
                    return;
                };
                if op_text != "||" {
                    return;
                }
                // Only flag if BOTH sides look like they could be nullish
                // (identifiers, member access, call). Otherwise `||` might
                // be intentional.
                let Some(left) = node.child_by_field_name("left") else {
                    return;
                };
                let Some(right) = node.child_by_field_name("right") else {
                    return;
                };
                if is_potentially_nullish(left) && is_potentially_nullish(right) {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "prefer-nullish-coalescing".into(),
                        severity: Severity::Minor,
                        message:
                            "Use `??` instead of `||` to allow valid falsy values like `0` or `''`."
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

fn is_potentially_nullish(node: Node) -> bool {
    matches!(
        node.kind(),
        "identifier"
            | "member_expression"
            | "call_expression"
            | "subscript_expression"
            | "optional_chain"
    )
}
