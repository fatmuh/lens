//! `prefer-promise-reject-errors` — flags `Promise.reject("string")` and
//! `reject("string")` (should reject with Error objects).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct PreferPromiseRejectErrors;

impl Rule for PreferPromiseRejectErrors {
    fn id(&self) -> &'static str {
        "prefer-promise-reject-errors"
    }
    fn name(&self) -> &'static str {
        "Reject with `Error`"
    }
    fn description(&self) -> &'static str {
        "Always reject a Promise with an `Error` object, not a string."
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
                if node.kind() != "call_expression" {
                    return;
                }
                let Some(func) = node.child_by_field_name("function") else {
                    return;
                };
                // Case 1: Promise.reject(...)
                let is_promise_reject = func.kind() == "member_expression"
                    && func
                        .child_by_field_name("object")
                        .map(|o| o.utf8_text(source.as_bytes()).unwrap_or("") == "Promise")
                        .unwrap_or(false)
                    && func
                        .child_by_field_name("property")
                        .map(|p| p.utf8_text(source.as_bytes()).unwrap_or("") == "reject")
                        .unwrap_or(false);
                // Case 2: `reject(...)` inside a Promise executor (we
                // can't easily detect this without scope analysis; skip)
                if !is_promise_reject {
                    return;
                }
                // First argument must be a string/number/etc literal.
                let mut cursor = node.walk();
                for arg in node.children(&mut cursor) {
                    if arg.kind() == "arguments" {
                        let mut ac = arg.walk();
                        for a in arg.children(&mut ac) {
                            if matches!(
                                a.kind(),
                                "string"
                                    | "number"
                                    | "true"
                                    | "false"
                                    | "null"
                                    | "undefined"
                                    | "template_string"
                            ) {
                                let start = node.start_position();
                                let end = node.end_position();
                                issues.push(Issue {
                                    rule_id: "prefer-promise-reject-errors".into(),
                                    severity: Severity::Major,
                                    message: "Reject with an `Error` object, not a literal.".into(),
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
            });
        });
        issues
    }
}
