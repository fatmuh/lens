//! `no-proto` — flags `obj.__proto__` (deprecated; use `Object.getPrototypeOf`).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoProto;

impl Rule for NoProto {
    fn id(&self) -> &'static str {
        "no-proto"
    }
    fn name(&self) -> &'static str {
        "No `__proto__`"
    }
    fn description(&self) -> &'static str {
        "Avoid `__proto__`; use `Object.getPrototypeOf` instead."
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
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
                if node.kind() != "member_expression" {
                    return;
                }
                let Some(prop) = node.child_by_field_name("property") else {
                    return;
                };
                let Ok(text) = prop.utf8_text(source.as_bytes()) else {
                    return;
                };
                if text == "__proto__" {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-proto".into(),
                        severity: Severity::Major,
                        message: "Use `Object.getPrototypeOf` instead of `__proto__`.".into(),
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
