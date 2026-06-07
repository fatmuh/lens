//! `no-lonely-if` — flags `if (x) { ... }` as the only statement in an
//! `else` branch. Use `else if` instead.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoLonelyIf;

impl Rule for NoLonelyIf {
    fn id(&self) -> &'static str { "no-lonely-if" }
    fn name(&self) -> &'static str { "No lonely `if` in `else`" }
    fn description(&self) -> &'static str {
        "Use `else if` instead of an `if` as the sole statement of an `else` branch."
    }
    fn default_severity(&self) -> Severity { Severity::Minor }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "else_clause" { return; }
                // Look for an if_statement anywhere under the else body
                // (it may be inside a statement_block).
                let mut found = false;
                crate::analyzer::parser::visit_descendants(node, |n| {
                    if n != node && n.kind() == "if_statement" {
                        found = true;
                    }
                });
                if found {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-lonely-if".into(),
                        severity: Severity::Minor,
                        message: "Use `else if` instead of nested `if` inside `else`.".into(),
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
