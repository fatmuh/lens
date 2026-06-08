//! `no-underscore-dangle` — flags identifiers that have a dangling
//! underscore at the beginning or end (except leading underscore for
//! intentionally-unused parameters, which is allowed by convention).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUnderscoreDangle;

impl Rule for NoUnderscoreDangle {
    fn id(&self) -> &'static str {
        "no-underscore-dangle"
    }
    fn name(&self) -> &'static str {
        "No dangling underscore"
    }
    fn description(&self) -> &'static str {
        "Don't end names with `_` (leading `_` is fine for unused params)."
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
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                let name = match node.kind() {
                    "variable_declarator" => node.child_by_field_name("name"),
                    "function_declaration" => node.child_by_field_name("name"),
                    "required_parameter" | "optional_parameter" => {
                        node.child_by_field_name("pattern")
                    }
                    _ => None,
                };
                let Some(name) = name else {
                    return;
                };
                let Ok(text) = name.utf8_text(source.as_bytes()) else {
                    return;
                };
                // Allow leading underscore (intentionally unused).
                let trailing_underscore = text.ends_with('_');
                if trailing_underscore && text != "_" {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-underscore-dangle".into(),
                        severity: Severity::Minor,
                        message: format!("`{}` ends with a dangling underscore.", text),
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
