//! `no-new-symbol` — flags `new Symbol()` (use `Symbol()` instead).

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoNewSymbol;

impl Rule for NoNewSymbol {
    fn id(&self) -> &'static str { "no-new-symbol" }
    fn name(&self) -> &'static str { "No `new Symbol`" }
    fn description(&self) -> &'static str {
        "`new Symbol()` throws; use `Symbol()` instead."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "new_expression" { return; }
                let Some(ctor) = node.child_by_field_name("constructor") else { return; };
                let Ok(name) = ctor.utf8_text(source.as_bytes()) else { return; };
                if name == "Symbol" {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-new-symbol".into(),
                        severity: Severity::Major,
                        message: "Use `Symbol()` instead of `new Symbol()`.".into(),
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
