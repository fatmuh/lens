//! `no-var` — flags `var` declarations; use `let` or `const`.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoVar;

impl Rule for NoVar {
    fn id(&self) -> &'static str { "no-var" }
    fn name(&self) -> &'static str { "No `var` keyword" }
    fn description(&self) -> &'static str {
        "Use `let` or `const` instead of `var`. `var` has function scope and hoisting issues."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                // tree-sitter-typescript: `var` shows up as a `variable_declaration`
                // whose first child is the `var` keyword token.
                if node.kind() == "variable_declaration" {
                    if let Some(first) = node.child(0) {
                        if first.kind() == "var" || first.kind() == "var_specifier" {
                            if let Ok(text) = first.utf8_text(source.as_bytes()) {
                                if text == "var" {
                                    let start = node.start_position();
                                    issues.push(Issue {
                                        rule_id: "no-var".into(),
                                        severity: Severity::Major,
                                        message: "Use `let` or `const` instead of `var`.".into(),
                                        file: file.path.clone(),
                                        start_line: start.row as u32 + 1,
                                        end_line: start.row as u32 + 1,
                                        start_column: start.column as u32,
                                        end_column: (start.column + 3) as u32,
                                    });
                                }
                            }
                        }
                    }
                }
            });
        });
        issues
    }
}
