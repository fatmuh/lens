//! `no-unreachable` — flags statements after `return`, `throw`, `break`,
//! `continue` in the same block.

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUnreachable;

impl Rule for NoUnreachable {
    fn id(&self) -> &'static str {
        "no-unreachable"
    }
    fn name(&self) -> &'static str {
        "No unreachable code"
    }
    fn description(&self) -> &'static str {
        "Code after `return`/`throw`/`break`/`continue` in the same block can never run."
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
            Language::Dart,
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "statement_block" {
                    return;
                }
                let mut saw_terminator = false;
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    let kind = child.kind();
                    if saw_terminator && !is_brace_or_comment(kind) {
                        let start = child.start_position();
                        let end = child.end_position();
                        issues.push(Issue {
                            rule_id: "no-unreachable".into(),
                            severity: Severity::Critical,
                            message: "Unreachable code after a return/throw/break/continue.".into(),
                            file: file.path.clone(),
                            start_line: start.row as u32 + 1,
                            end_line: end.row as u32 + 1,
                            start_column: start.column as u32,
                            end_column: end.column as u32,
                        });
                        // Only report the first unreachable per block.
                        return;
                    }
                    if matches!(
                        kind,
                        "return_statement"
                            | "throw_statement"
                            | "break_statement"
                            | "continue_statement"
                    ) {
                        saw_terminator = true;
                    }
                }
            });
        });
        issues
    }
}

fn is_brace_or_comment(kind: &str) -> bool {
    matches!(
        kind,
        "{" | "}" | "(" | ")" | "[" | "]" | "comment" | "ERROR"
    )
}
