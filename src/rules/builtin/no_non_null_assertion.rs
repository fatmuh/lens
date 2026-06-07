//! `no-non-null-assertion` — flags `!` after an expression (TypeScript non-null
//! assertion). Prefer explicit null checks.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoNonNullAssertion;

impl Rule for NoNonNullAssertion {
    fn id(&self) -> &'static str { "no-non-null-assertion" }
    fn name(&self) -> &'static str { "No non-null assertion" }
    fn description(&self) -> &'static str {
        "Avoid `!` non-null assertions. Use explicit null checks instead."
    }
    fn default_severity(&self) -> Severity { Severity::Minor }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        if !matches!(lang, Language::TypeScript | Language::Tsx) { return issues; }
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() == "non_null_expression" {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-non-null-assertion".into(),
                        severity: Severity::Minor,
                        message: "Avoid `!` non-null assertions; check for `null`/`undefined` explicitly.".into(),
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
