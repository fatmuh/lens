//! `no-new-buffer` — flags `new Buffer(...)` (deprecated since Node 4;
//! use `Buffer.from(...)` or `Buffer.alloc(...)`).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoNewBuffer;

impl Rule for NoNewBuffer {
    fn id(&self) -> &'static str {
        "no-new-buffer"
    }
    fn name(&self) -> &'static str {
        "No `new Buffer`"
    }
    fn description(&self) -> &'static str {
        "Use `Buffer.from(...)` or `Buffer.alloc(...)` instead of `new Buffer(...)`."
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
                if node.kind() != "new_expression" {
                    return;
                }
                let Some(ctor) = node.child_by_field_name("constructor") else {
                    return;
                };
                let Ok(name) = ctor.utf8_text(source.as_bytes()) else {
                    return;
                };
                if name != "Buffer" {
                    return;
                }
                let start = node.start_position();
                let end = node.end_position();
                issues.push(Issue {
                    rule_id: "no-new-buffer".into(),
                    severity: Severity::Major,
                    message: "Use `Buffer.from(...)` instead of `new Buffer(...)`.".into(),
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
