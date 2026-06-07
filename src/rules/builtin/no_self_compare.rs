//! `no-self-compare` — flags `x === x` or `x == x` (almost always a bug).

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoSelfCompare;

impl Rule for NoSelfCompare {
    fn id(&self) -> &'static str { "no-self-compare" }
    fn name(&self) -> &'static str { "No self-comparison" }
    fn description(&self) -> &'static str {
        "Comparing a value to itself (`x === x`) is almost always a bug."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "binary_expression" { return; }
                let Some(op) = node.child_by_field_name("operator") else { return; };
                let op_text = match op.utf8_text(source.as_bytes()) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                if !matches!(op_text, "==" | "===" | "!=" | "!==" | "<" | ">" | "<=" | ">=") { return; }
                let left = node.child_by_field_name("left");
                let right = node.child_by_field_name("right");
                let (Some(l), Some(r)) = (left, right) else { return; };
                let l_text = l.utf8_text(source.as_bytes()).unwrap_or("");
                let r_text = r.utf8_text(source.as_bytes()).unwrap_or("");
                if l_text == r_text && !l_text.is_empty() {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-self-compare".into(),
                        severity: Severity::Major,
                        message: "Comparing a value to itself is almost always a bug.".into(),
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
