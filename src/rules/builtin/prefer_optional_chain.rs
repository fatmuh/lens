//! `prefer-optional-chain` — flags `a && a.b && a.b.c` patterns that could
//! be `a?.b?.c`.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct PreferOptionalChain;

impl Rule for PreferOptionalChain {
    fn id(&self) -> &'static str { "prefer-optional-chain" }
    fn name(&self) -> &'static str { "Prefer optional chaining" }
    fn description(&self) -> &'static str {
        "Use `a?.b?.c` instead of `a && a.b && a.b.c`."
    }
    fn default_severity(&self) -> Severity { Severity::Minor }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        if !matches!(lang, Language::TypeScript | Language::Tsx) { return issues; }
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if !matches!(node.kind(), "binary_expression" | "logical_expression") { return; }
                let Some(op) = node.child_by_field_name("operator") else { return; };
                let Ok(op_text) = op.utf8_text(source.as_bytes()) else { return; };
                if op_text != "&&" { return; }
                // Check if the right-hand side is `LHS.b` (member access on
                // the LHS). That's the classic "a && a.b" pattern.
                let Some(left) = node.child_by_field_name("left") else { return; };
                let Some(right) = node.child_by_field_name("right") else { return; };
                if right.kind() == "member_expression" {
                    if let Some(obj) = right.child_by_field_name("object") {
                        let Ok(obj_text) = obj.utf8_text(source.as_bytes()) else { return; };
                        let Ok(left_text) = left.utf8_text(source.as_bytes()) else { return; };
                        // If the right's object is the left, or matches by name.
                        if obj_text == left_text || obj_text.trim_start_matches('(').trim_end_matches(')') == left_text {
                            let start = node.start_position();
                            let end = node.end_position();
                            issues.push(Issue {
                                rule_id: "prefer-optional-chain".into(),
                                severity: Severity::Minor,
                                message: "Use `?.` optional chaining instead of `&&` truthiness checks.".into(),
                                file: file.path.clone(),
                                start_line: start.row as u32 + 1,
                                end_line: end.row as u32 + 1,
                                start_column: start.column as u32,
                                end_column: end.column as u32,
                            });
                        }
                    }
                }
            });
        });
        issues
    }
}
