//! `no-await-in-loop` — flags `await` inside a `for`/`while` loop.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoAwaitInLoop;

impl Rule for NoAwaitInLoop {
    fn id(&self) -> &'static str {
        "no-await-in-loop"
    }
    fn name(&self) -> &'static str {
        "No `await` in loop"
    }
    fn description(&self) -> &'static str {
        "Avoid `await` inside a loop; await in parallel with `Promise.all` is usually faster."
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
                if node.kind() != "await_expression" {
                    return;
                }
                if let Some(loop_node) = enclosing_loop(node) {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-await-in-loop".into(),
                        severity: Severity::Minor,
                        message:
                            "`await` in a loop is sequential; use `Promise.all` to parallelize."
                                .into(),
                        file: file.path.clone(),
                        start_line: start.row as u32 + 1,
                        end_line: end.row as u32 + 1,
                        start_column: start.column as u32,
                        end_column: end.column as u32,
                    });
                    let _ = loop_node;
                }
            });
        });
        issues
    }
}

fn enclosing_loop(node: Node) -> Option<Node> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if matches!(
            parent.kind(),
            "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
        ) {
            return Some(parent);
        }
        if matches!(
            parent.kind(),
            "function_declaration"
                | "function"
                | "arrow_function"
                | "method_definition"
                | "generator_function_declaration"
        ) {
            return None;
        }
        current = parent;
    }
    None
}
