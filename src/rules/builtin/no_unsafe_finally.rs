//! `no-unsafe-finally` — flags `return`/`throw`/`break`/`continue` inside
//! a `finally` block, which suppresses exceptions from the try/catch.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUnsafeFinally;

impl Rule for NoUnsafeFinally {
    fn id(&self) -> &'static str {
        "no-unsafe-finally"
    }
    fn name(&self) -> &'static str {
        "Unsafe control in `finally`"
    }
    fn description(&self) -> &'static str {
        "Don't use `return`/`throw`/`break`/`continue` in `finally` — it can swallow exceptions."
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
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "finally_clause" {
                    return;
                }
                // Walk the body, flag terminators.
                if let Some(body) = node.child_by_field_name("body") {
                    walk_for_terminators(body, file, source, &mut issues);
                }
            });
        });
        issues
    }
}

fn walk_for_terminators(node: Node, file: &FileAnalysis, source: &str, issues: &mut Vec<Issue>) {
    let mut cursor = node.walk();
    for c in node.children(&mut cursor) {
        if matches!(
            c.kind(),
            "return_statement" | "throw_statement" | "break_statement" | "continue_statement"
        ) {
            let start = c.start_position();
            let end = c.end_position();
            issues.push(Issue {
                rule_id: "no-unsafe-finally".into(),
                severity: Severity::Critical,
                message: format!(
                    "`{}` in a `finally` block can swallow exceptions.",
                    c.kind().replace("_statement", "")
                ),
                file: file.path.clone(),
                start_line: start.row as u32 + 1,
                end_line: end.row as u32 + 1,
                start_column: start.column as u32,
                end_column: end.column as u32,
            });
        }
        // Recurse into nested blocks.
        if matches!(
            c.kind(),
            "statement_block"
                | "if_statement"
                | "switch_statement"
                | "for_statement"
                | "while_statement"
                | "try_statement"
        ) {
            walk_for_terminators(c, file, source, issues);
        }
    }
}
