//! `require-await` — flags `async` functions that don't use `await`.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct RequireAwait;

impl Rule for RequireAwait {
    fn id(&self) -> &'static str {
        "require-await"
    }
    fn name(&self) -> &'static str {
        "Require `await` in async"
    }
    fn description(&self) -> &'static str {
        "`async` functions that don't `await` anything can be made synchronous."
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
                if !matches!(
                    node.kind(),
                    "function_declaration"
                        | "function"
                        | "method_definition"
                        | "arrow_function"
                        | "generator_function_declaration"
                ) {
                    return;
                }
                // Check if the function is async. In tree-sitter-typescript,
                // `async` shows up as a token child of arrow_function /
                // function_declaration / method_definition.
                let text = match node.utf8_text(source.as_bytes()) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                if !text.trim_start().starts_with("async") {
                    return;
                }
                // Check for any `await` inside the function body.
                let body = match node.child_by_field_name("body") {
                    Some(b) => b,
                    None => return, // abstract method — no body
                };
                if !has_await(body) {
                    let start = node.start_position();
                    let end = node.end_position();
                    let name = node
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("<anonymous>");
                    issues.push(Issue {
                        rule_id: "require-await".into(),
                        severity: Severity::Major,
                        message: format!(
                            "Async function `{}` has no `await`; remove `async`.",
                            name
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

fn has_await(node: Node) -> bool {
    let mut found = false;
    crate::analyzer::parser::visit_descendants(node, |n| {
        if found {
            return;
        }
        if n.kind() == "await_expression" {
            found = true;
        }
    });
    found
}
