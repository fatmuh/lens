//! `prefer-spread` — flags `[].concat(a, b)` (use `[...a, ...b]` instead).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct PreferSpread;

impl Rule for PreferSpread {
    fn id(&self) -> &'static str {
        "prefer-spread"
    }
    fn name(&self) -> &'static str {
        "Prefer spread over concat"
    }
    fn description(&self) -> &'static str {
        "Use `[...a, ...b]` instead of `[].concat(a, b)`."
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
                if node.kind() != "call_expression" {
                    return;
                }
                let Some(func) = node.child_by_field_name("function") else {
                    return;
                };
                if func.kind() != "member_expression" {
                    return;
                }
                let Some(obj) = func.child_by_field_name("object") else {
                    return;
                };
                let Some(prop) = func.child_by_field_name("property") else {
                    return;
                };
                let Ok(obj_text) = obj.utf8_text(source.as_bytes()) else {
                    return;
                };
                let Ok(prop_text) = prop.utf8_text(source.as_bytes()) else {
                    return;
                };
                if obj_text == "[]" && prop_text == "concat" {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "prefer-spread".into(),
                        severity: Severity::Minor,
                        message: "Use `[...a, ...b]` instead of `[].concat(a, b)`.".into(),
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
