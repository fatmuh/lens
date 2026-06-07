//! `no-empty-function` — flags functions/methods with an empty body.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoEmptyFunction;

impl Rule for NoEmptyFunction {
    fn id(&self) -> &'static str { "no-empty-function" }
    fn name(&self) -> &'static str { "No empty functions" }
    fn description(&self) -> &'static str {
        "Functions with empty bodies are usually a mistake or work-in-progress."
    }
    fn default_severity(&self) -> Severity { Severity::Minor }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if !matches!(node.kind(), "function_declaration" | "function" | "method_definition"
                    | "arrow_function" | "generator_function_declaration") {
                    return;
                }
                // Find body
                let body = node.child_by_field_name("body");
                let Some(body) = body else { return; };
                if body.kind() == "statement_block" {
                    // Count non-comment, non-brace children.
                    let mut real = 0;
                    let mut cursor = body.walk();
                    for c in body.children(&mut cursor) {
                        if !matches!(c.kind(), "comment" | "ERROR" | "{" | "}") {
                            real += 1;
                        }
                    }
                    if real == 0 {
                        let start = node.start_position();
                        let end = node.end_position();
                        issues.push(Issue {
                            rule_id: "no-empty-function".into(),
                            severity: Severity::Minor,
                            message: "Function has an empty body.".into(),
                            file: file.path.clone(),
                            start_line: start.row as u32 + 1,
                            end_line: end.row as u32 + 1,
                            start_column: start.column as u32,
                            end_column: end.column as u32,
                        });
                    }
                }
            });
        });
        issues
    }
}
