//! `no-else-return` — flags `if (x) return; else y` where the `else` is unnecessary.

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoElseReturn;

impl Rule for NoElseReturn {
    fn id(&self) -> &'static str {
        "no-else-return"
    }
    fn name(&self) -> &'static str {
        "No `else` after `return`"
    }
    fn description(&self) -> &'static str {
        "Don't use `else` after a `return`; the `else` is unnecessary."
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
                if node.kind() != "if_statement" {
                    return;
                }
                // The "then" body must be a statement_block containing a
                // `return_statement` (or other early-exit) as the LAST
                // statement. The if must also have an else_clause.
                let Some(consequence) = node.child_by_field_name("consequence") else {
                    return;
                };
                let has_else = node
                    .children(&mut node.walk())
                    .any(|c| c.kind() == "else_clause");
                if !has_else {
                    return;
                }
                // The consequence must end with a return/throw/break/continue.
                let ends_with_exit = if consequence.kind() == "statement_block" {
                    let mut cursor = consequence.walk();
                    consequence
                        .children(&mut cursor)
                        .last()
                        .map_or(false, |last| {
                            matches!(
                                last.kind(),
                                "return_statement"
                                    | "throw_statement"
                                    | "break_statement"
                                    | "continue_statement"
                            )
                        })
                } else {
                    matches!(
                        consequence.kind(),
                        "return_statement"
                            | "throw_statement"
                            | "break_statement"
                            | "continue_statement"
                    )
                };
                if ends_with_exit {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-else-return".into(),
                        severity: Severity::Minor,
                        message: "Drop the `else`; the `if` branch already returns.".into(),
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
