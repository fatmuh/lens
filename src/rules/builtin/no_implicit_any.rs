//! `no-implicit-any` — flags function parameters without a type annotation.
//! TypeScript should require explicit types in `strict` mode; this rule
//! catches cases where strict mode is off and types are missing.

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoImplicitAny;

impl Rule for NoImplicitAny {
    fn id(&self) -> &'static str {
        "no-implicit-any"
    }
    fn name(&self) -> &'static str {
        "No implicit `any`"
    }
    fn description(&self) -> &'static str {
        "Function parameters should have explicit type annotations."
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::TypeScript, Language::Tsx]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        if !matches!(lang, Language::TypeScript | Language::Tsx) {
            return issues;
        }
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if !matches!(
                    node.kind(),
                    "function_declaration"
                        | "function"
                        | "method_definition"
                        | "generator_function_declaration"
                        | "arrow_function"
                ) {
                    return;
                }
                let Some(params) = node.child_by_field_name("parameters") else {
                    return;
                };
                let mut cursor = params.walk();
                for c in params.children(&mut cursor) {
                    if !matches!(c.kind(), "required_parameter" | "optional_parameter") {
                        continue;
                    }
                    // required_parameter: pattern + (optional type_annotation)
                    let has_type = c.child_by_field_name("type").is_some();
                    if !has_type {
                        let start = c.start_position();
                        let end = c.end_position();
                        let name = c
                            .child_by_field_name("pattern")
                            .and_then(|p| p.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("?");
                        issues.push(Issue {
                            rule_id: "no-implicit-any".into(),
                            severity: Severity::Minor,
                            message: format!("Parameter `{}` is missing a type annotation.", name),
                            file: file.path.clone(),
                            start_line: start.row as u32 + 1,
                            end_line: end.row as u32 + 1,
                            start_column: start.column as u32,
                            end_column: end.column as u32,
                        });
                    }
                }
            });
        });
        issues
    }
}
