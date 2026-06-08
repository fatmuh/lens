//! `no-eqeqeq` — flags `==` and `!=`; use `===` / `!==`.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoEqeqeq;

impl Rule for NoEqeqeq {
    fn id(&self) -> &'static str {
        "no-eqeqeq"
    }
    fn name(&self) -> &'static str {
        "Use `===` and `!==`"
    }
    fn description(&self) -> &'static str {
        "Use strict equality (`===` / `!==`) instead of loose equality (`==` / `!=`)."
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
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
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if let Some(op) = binary_op_text(node, source) {
                    if op == "==" || op == "!=" {
                        let start = node.start_position();
                        issues.push(Issue {
                            rule_id: "no-eqeqeq".into(),
                            severity: Severity::Major,
                            message: format!(
                                "Use `{}` instead of `{}`.",
                                if op == "==" { "===" } else { "!==" },
                                op
                            ),
                            file: file.path.clone(),
                            start_line: start.row as u32 + 1,
                            end_line: start.row as u32 + 1,
                            start_column: start.column as u32,
                            end_column: (start.column + op.len()) as u32,
                        });
                    }
                }
            });
        });
        issues
    }
}

fn binary_op_text(node: Node, source: &str) -> Option<String> {
    if node.kind() != "binary_expression" {
        return None;
    }
    let op_node = node.child_by_field_name("operator")?;
    op_node.utf8_text(source.as_bytes()).ok().map(String::from)
}
