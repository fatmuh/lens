//! `no-useless-return` — flags a bare `return;` at the end of a function.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUselessReturn;

impl Rule for NoUselessReturn {
    fn id(&self) -> &'static str {
        "no-useless-return"
    }
    fn name(&self) -> &'static str {
        "No useless return"
    }
    fn description(&self) -> &'static str {
        "Don't end a function with a bare `return;` — it's implicit."
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
            Language::Dart,
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |block| {
                if block.kind() != "statement_block" {
                    return;
                }
                let mut cursor = block.walk();
                let children: Vec<Node> = block.children(&mut cursor).collect();
                // Skip braces — the last "real" child might be preceded by `}`.
                if let Some(last) = children
                    .iter()
                    .rev()
                    .find(|c| !matches!(c.kind(), "{" | "}"))
                {
                    if last.kind() == "return_statement" {
                        if let Ok(text) = last.utf8_text(source.as_bytes()) {
                            // Bare `return;` (no value, with or without semicolon)
                            let t = text.trim();
                            if t == "return" || t == "return;" || t == "return ;" {
                                let start = last.start_position();
                                let end = last.end_position();
                                issues.push(Issue {
                                    rule_id: "no-useless-return".into(),
                                    severity: Severity::Minor,
                                    message: "Useless `return;` at the end of a function.".into(),
                                    file: file.path.clone(),
                                    start_line: start.row as u32 + 1,
                                    end_line: end.row as u32 + 1,
                                    start_column: start.column as u32,
                                    end_column: end.column as u32,
                                });
                            }
                        }
                    }
                }
            });
        });
        issues
    }
}
