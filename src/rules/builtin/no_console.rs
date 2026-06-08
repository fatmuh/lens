//! `no-console` — flags `console.log/info/warn/error/debug` calls in source files.
//! Test files are skipped (they often need to log).

use std::path::Path;

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};

pub struct NoConsole;

impl Rule for NoConsole {
    fn id(&self) -> &'static str {
        "no-console"
    }
    fn name(&self) -> &'static str {
        "No `console.*` calls"
    }
    fn description(&self) -> &'static str {
        "Avoid using `console.log` etc. in production code. Use a proper logger."
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        // Skip test files — logging is expected there.
        let path_str = file.path.to_string_lossy();
        if is_test_file(&file.path)
            || path_str.contains(".spec.")
            || path_str.contains(".test.")
            || path_str.contains("__tests__")
        {
            return Vec::new();
        }
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() == "call_expression" {
                    if is_console_call(node, source) {
                        let start = node.start_position();
                        let end = node.end_position();
                        issues.push(Issue {
                            rule_id: "no-console".into(),
                            severity: Severity::Minor,
                            message: "Avoid `console.*` calls; use a proper logger.".into(),
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

fn is_test_file(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.ends_with(".test.ts")
        || s.ends_with(".test.tsx")
        || s.ends_with(".spec.ts")
        || s.ends_with(".spec.tsx")
        || s.contains("__tests__/")
}

fn is_console_call(node: Node, source: &str) -> bool {
    if let Some(func) = node.child_by_field_name("function") {
        if func.kind() == "member_expression" {
            if let Some(obj) = func.child_by_field_name("object") {
                if let Ok(text) = obj.utf8_text(source.as_bytes()) {
                    return text == "console";
                }
            }
        }
    }
    false
}
