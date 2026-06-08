//! `prefer-arrow-callback` — flags `function () { ... }` passed as a callback
//! where an arrow function would be equivalent.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct PreferArrowCallback;

impl Rule for PreferArrowCallback {
    fn id(&self) -> &'static str {
        "prefer-arrow-callback"
    }
    fn name(&self) -> &'static str {
        "Prefer arrow callbacks"
    }
    fn description(&self) -> &'static str {
        "Use arrow functions for callbacks: `arr.map(x => x + 1)` instead of `arr.map(function (x) { return x + 1; })`."
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
            // Look for `function (...) {}` directly inside an `arguments` node
            // (i.e. as an argument to a call).
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if !matches!(
                    node.kind(),
                    "function" | "generator_function" | "function_declaration"
                ) {
                    return;
                }
                if !is_callback_position(node) {
                    return;
                }
                // Skip generators and named function expressions (named is
                // fine but we leave it for now).
                if node.kind() != "function" {
                    return;
                }
                let start = node.start_position();
                let end = node.end_position();
                issues.push(Issue {
                    rule_id: "prefer-arrow-callback".into(),
                    severity: Severity::Minor,
                    message: "Use an arrow function for this callback.".into(),
                    file: file.path.clone(),
                    start_line: start.row as u32 + 1,
                    end_line: end.row as u32 + 1,
                    start_column: start.column as u32,
                    end_column: end.column as u32,
                });
            });
        });
        issues
    }
}

/// True if this `function` is a direct argument to a call (i.e. its
/// grandparent is a `call_expression` or `new_expression`).
fn is_callback_position(func: Node) -> bool {
    let Some(parent) = func.parent() else {
        return false;
    };
    // Walk up: arguments → call_expression
    parent.parent().map_or(false, |gp| {
        matches!(gp.kind(), "call_expression" | "new_expression")
    })
}
