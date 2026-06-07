//! `max-params` — flags functions/methods with more than a threshold of
//! parameters (default 5). Long parameter lists are a smell — usually a
//! missing options object or builder.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct MaxParams;

const DEFAULT_THRESHOLD: u32 = 5;

impl Rule for MaxParams {
    fn id(&self) -> &'static str { "max-params" }
    fn name(&self) -> &'static str { "Too many parameters" }
    fn description(&self) -> &'static str {
        "Functions with more than 5 parameters are hard to call. Consider an options object."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if !matches!(node.kind(), "function_declaration" | "function" | "method_definition"
                    | "generator_function_declaration") {
                    return;
                }
                let Some(params) = node.child_by_field_name("parameters") else { return; };
                let count = count_params(params, source);
                if count > DEFAULT_THRESHOLD {
                    let start = node.start_position();
                    let end = node.end_position();
                    let name = node.child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("<anonymous>");
                    issues.push(Issue {
                        rule_id: "max-params".into(),
                        severity: Severity::Major,
                        message: format!(
                            "Function `{}` has {} parameters (max {}).",
                            name, count, DEFAULT_THRESHOLD
                        ),
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

fn count_params(params: Node, source: &str) -> u32 {
    let mut count = 0u32;
    let mut cursor = params.walk();
    for c in params.children(&mut cursor) {
        if matches!(c.kind(), "required_parameter" | "optional_parameter" | "rest_pattern"
            | "assignment_pattern" | "identifier") {
            // Don't count a single "this" or rest indicator.
            if let Ok(text) = c.utf8_text(source.as_bytes()) {
                if text == "this" { continue; }
            }
            count += 1;
        }
    }
    count
}
