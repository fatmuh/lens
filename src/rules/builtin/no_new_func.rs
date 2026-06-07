//! `no-new-func` — flags `new Function(...)`. Same security risk as eval.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoNewFunc;

impl Rule for NoNewFunc {
    fn id(&self) -> &'static str { "no-new-func" }
    fn name(&self) -> &'static str { "No `new Function()`" }
    fn description(&self) -> &'static str {
        "Avoid `new Function(...)`. It's equivalent to `eval` and a security risk."
    }
    fn default_severity(&self) -> Severity { Severity::Blocker }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() == "new_expression" {
                    if let Some(constructor) = node.child_by_field_name("constructor") {
                        if constructor.kind() == "identifier" {
                            if let Ok(text) = constructor.utf8_text(source.as_bytes()) {
                                if text == "Function" {
                                    let start = node.start_position();
                                    let end = node.end_position();
                                    issues.push(Issue {
                                        rule_id: "no-new-func".into(),
                                        severity: Severity::Blocker,
                                        message: "Avoid `new Function(...)`; it's equivalent to `eval`.".into(),
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
            });
        });
        issues
    }
}
