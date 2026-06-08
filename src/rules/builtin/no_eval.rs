//! `no-eval` — flags `eval()` calls. eval is a code injection vector.

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoEval;

impl Rule for NoEval {
    fn id(&self) -> &'static str {
        "no-eval"
    }
    fn name(&self) -> &'static str {
        "No `eval()`"
    }
    fn description(&self) -> &'static str {
        "Avoid `eval()`. It executes arbitrary code and is a security risk."
    }
    fn default_severity(&self) -> Severity {
        Severity::Blocker
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
                if node.kind() == "call_expression" {
                    if let Some(func) = node.child_by_field_name("function") {
                        if func.kind() == "identifier" {
                            if let Ok(text) = func.utf8_text(source.as_bytes()) {
                                if text == "eval" {
                                    let start = node.start_position();
                                    let end = node.end_position();
                                    issues.push(Issue {
                                        rule_id: "no-eval".into(),
                                        severity: Severity::Blocker,
                                        message: "Avoid `eval()`; it's a code-injection vector."
                                            .into(),
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
                }
            });
        });
        issues
    }
}
