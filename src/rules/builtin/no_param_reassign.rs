//! `no-param-reassign` — flags reassigning function parameters.

use std::collections::HashSet;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoParamReassign;

impl Rule for NoParamReassign {
    fn id(&self) -> &'static str {
        "no-param-reassign"
    }
    fn name(&self) -> &'static str {
        "No parameter reassignment"
    }
    fn description(&self) -> &'static str {
        "Don't reassign function parameters. Use a local variable instead."
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
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |func| {
                if !matches!(
                    func.kind(),
                    "function_declaration"
                        | "function"
                        | "method_definition"
                        | "arrow_function"
                        | "generator_function_declaration"
                ) {
                    return;
                }
                // Collect parameter names.
                let Some(params) = func.child_by_field_name("parameters") else {
                    return;
                };
                let mut param_names: HashSet<String> = HashSet::new();
                let mut cursor = params.walk();
                for c in params.children(&mut cursor) {
                    if matches!(
                        c.kind(),
                        "required_parameter" | "optional_parameter" | "identifier"
                    ) {
                        let name = c.child_by_field_name("pattern").or_else(|| {
                            if c.kind() == "identifier" {
                                Some(c)
                            } else {
                                None
                            }
                        });
                        if let Some(n) = name {
                            if let Ok(text) = n.utf8_text(source.as_bytes()) {
                                if text != "this" {
                                    param_names.insert(text.to_string());
                                }
                            }
                        }
                    }
                }
                if param_names.is_empty() {
                    return;
                }
                // Walk the function body and look for assignments to a param.
                crate::analyzer::parser::visit_descendants(func, |n| {
                    if n.kind() == "assignment_expression" {
                        if let Some(left) = n.child_by_field_name("left") {
                            if let Ok(text) = left.utf8_text(source.as_bytes()) {
                                if param_names.iter().any(|p| p == &text) {
                                    let start = n.start_position();
                                    let end = n.end_position();
                                    issues.push(Issue {
                                        rule_id: "no-param-reassign".into(),
                                        severity: Severity::Major,
                                        message: format!(
                                            "Don't reassign parameter `{}`; use a local variable.",
                                            text
                                        ),
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
                    if n.kind() == "update_expression" {
                        if let Some(arg) = n.child_by_field_name("argument") {
                            if let Ok(text) = arg.utf8_text(source.as_bytes()) {
                                if param_names.iter().any(|p| p == &text) {
                                    let start = n.start_position();
                                    let end = n.end_position();
                                    issues.push(Issue {
                                        rule_id: "no-param-reassign".into(),
                                        severity: Severity::Major,
                                        message: format!(
                                            "Don't mutate parameter `{}`; use a local variable.",
                                            text
                                        ),
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
        });
        issues
    }
}
