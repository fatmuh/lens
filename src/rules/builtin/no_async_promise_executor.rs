//! `no-async-promise-executor` — flags `new Promise(async (...) => {...})`.
//! The executor runs synchronously, so an async function there throws errors
//! in unhandled rejections.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoAsyncPromiseExecutor;

impl Rule for NoAsyncPromiseExecutor {
    fn id(&self) -> &'static str {
        "no-async-promise-executor"
    }
    fn name(&self) -> &'static str {
        "No async Promise executor"
    }
    fn description(&self) -> &'static str {
        "`new Promise(async ...)` runs the executor sync; errors become unhandled rejections."
    }
    fn default_severity(&self) -> Severity {
        Severity::Critical
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
                if node.kind() != "new_expression" {
                    return;
                }
                let Some(ctor) = node.child_by_field_name("constructor") else {
                    return;
                };
                let Ok(ctor_text) = ctor.utf8_text(source.as_bytes()) else {
                    return;
                };
                if ctor_text != "Promise" {
                    return;
                }
                // The text of the new expression contains "async" if and
                // only if its first argument is an async function. Cheap
                // and robust (avoids brittle AST shape assumptions).
                let Ok(text) = node.utf8_text(source.as_bytes()) else {
                    return;
                };
                // Strip the leading "new Promise" and any wrapping paren.
                let stripped = text
                    .trim_start_matches("new Promise")
                    .trim_start()
                    .trim_start_matches('(')
                    .trim_start();
                if stripped.starts_with("async") {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-async-promise-executor".into(),
                        severity: Severity::Critical,
                        message: "Don't use an `async` function as a Promise executor.".into(),
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

fn child_text_contains(node: Node, needle: &str, source: &str) -> bool {
    node.utf8_text(source.as_bytes())
        .map(|t| t.contains(needle))
        .unwrap_or(false)
}
