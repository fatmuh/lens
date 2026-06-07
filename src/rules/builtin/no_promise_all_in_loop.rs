//! `no-promise-all-in-loop` — flags `Promise.all` inside a for/while loop.
//! This usually indicates sequential awaits that should be parallel.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoPromiseAllInLoop;

impl Rule for NoPromiseAllInLoop {
    fn id(&self) -> &'static str { "no-promise-all-in-loop" }
    fn name(&self) -> &'static str { "No `Promise.all` in loop" }
    fn description(&self) -> &'static str {
        "Avoid `Promise.all` inside a loop. Build a list and `Promise.all` outside the loop instead."
    }
    fn default_severity(&self) -> Severity { Severity::Minor }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            // Track loop ancestors: when we see Promise.all, walk up to
            // see if we're inside a loop.
            let root = tree.root_node();
            let all_promise_alls: Vec<Node> = collect_descendants_of_kind(root, "call_expression")
                .into_iter()
                .filter(|n| is_promise_all(*n, source))
                .collect();
            for call in all_promise_alls {
                if has_loop_ancestor(call) {
                    let start = call.start_position();
                    let end = call.end_position();
                    issues.push(Issue {
                        rule_id: "no-promise-all-in-loop".into(),
                        severity: Severity::Minor,
                        message: "`Promise.all` inside a loop; collect promises first, await once.".into(),
                        file: file.path.clone(),
                        start_line: start.row as u32 + 1,
                        end_line: end.row as u32 + 1,
                        start_column: start.column as u32,
                        end_column: end.column as u32,
                    });
                }
            }
        });
        issues
    }
}

fn collect_descendants_of_kind<'a>(root: Node<'a>, kind: &str) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    crate::analyzer::parser::visit_descendants(root, |n| {
        if n.kind() == kind {
            out.push(n);
        }
    });
    out
}

fn is_promise_all(node: Node, source: &str) -> bool {
    if node.kind() != "call_expression" { return false; }
    let Some(func) = node.child_by_field_name("function") else { return false; };
    if func.kind() != "member_expression" { return false; }
    let Some(obj) = func.child_by_field_name("object") else { return false; };
    let Some(prop) = func.child_by_field_name("property") else { return false; };
    let Ok(obj_text) = obj.utf8_text(source.as_bytes()) else { return false; };
    let Ok(prop_text) = prop.utf8_text(source.as_bytes()) else { return false; };
    obj_text == "Promise" && prop_text == "all"
}

fn has_loop_ancestor(mut node: Node) -> bool {
    while let Some(parent) = node.parent() {
        if matches!(parent.kind(), "for_statement" | "for_in_statement" | "while_statement" | "do_statement") {
            return true;
        }
        // Stop at function boundary.
        if matches!(parent.kind(), "function_declaration" | "function" | "arrow_function" | "method_definition" | "generator_function_declaration") {
            return false;
        }
        node = parent;
    }
    false
}
