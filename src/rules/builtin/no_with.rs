//! `no-with` — flags `with` statements (forbidden in strict mode).

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoWith;

impl Rule for NoWith {
    fn id(&self) -> &'static str { "no-with" }
    fn name(&self) -> &'static str { "No `with` statement" }
    fn description(&self) -> &'static str {
        "Don't use `with` — forbidden in strict mode and a source of bugs."
    }
    fn default_severity(&self) -> Severity { Severity::Critical }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() == "with_statement" {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-with".into(),
                        severity: Severity::Critical,
                        message: "Don't use `with`; it's forbidden in strict mode.".into(),
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
