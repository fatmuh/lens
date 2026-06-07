//! `no-unneeded-ternary` — flags `x ? true : false` (use `x` directly) and
//! `x ? false : true` (use `!x`).

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUnneededTernary;

impl Rule for NoUnneededTernary {
    fn id(&self) -> &'static str { "no-unneeded-ternary" }
    fn name(&self) -> &'static str { "No unneeded ternary" }
    fn description(&self) -> &'static str {
        "Don't use `x ? true : false` or `x ? false : true` — use `x` or `!x`."
    }
    fn default_severity(&self) -> Severity { Severity::Minor }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "ternary_expression" { return; }
                let Some(consequence) = node.child_by_field_name("consequence") else { return; };
                let Some(alternative) = node.child_by_field_name("alternative") else { return; };
                let Ok(c) = consequence.utf8_text(source.as_bytes()) else { return; };
                let Ok(a) = alternative.utf8_text(source.as_bytes()) else { return; };
                if (c == "true" && a == "false") || (c == "false" && a == "true") {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-unneeded-ternary".into(),
                        severity: Severity::Minor,
                        message: if c == "true" { "Use the condition directly instead of `cond ? true : false`.".into() }
                                 else { "Negate the condition directly instead of `cond ? false : true`.".into() },
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
