//! `no-return-await` — flags `return await x` (unnecessary; just `return x`).

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoReturnAwait;

impl Rule for NoReturnAwait {
    fn id(&self) -> &'static str {
        "no-return-await"
    }
    fn name(&self) -> &'static str {
        "No `return await`"
    }
    fn description(&self) -> &'static str {
        "Inside an `async` function, `return await x` is the same as `return x`. Drop the `await`."
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
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "return_statement" {
                    return;
                }
                // Check that the function we're in is async.
                let Some(func) = enclosing_async_function(node) else {
                    return;
                };
                if !is_async(func, source) {
                    return;
                }
                // Find an await_expression child.
                let mut has_await = false;
                crate::analyzer::parser::visit_descendants(node, |n| {
                    if n.kind() == "await_expression" {
                        has_await = true;
                    }
                });
                if has_await {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-return-await".into(),
                        severity: Severity::Minor,
                        message: "Drop the unnecessary `await` from this `return` statement."
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

fn enclosing_async_function(node: Node) -> Option<Node> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if matches!(
            parent.kind(),
            "function_declaration"
                | "function"
                | "method_definition"
                | "arrow_function"
                | "generator_function_declaration"
        ) {
            return Some(parent);
        }
        current = parent;
    }
    None
}

fn is_async(func: Node, source: &str) -> bool {
    func.utf8_text(source.as_bytes())
        .map(|t| t.trim_start().starts_with("async"))
        .unwrap_or(false)
}
