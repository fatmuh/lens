//! `quote-props` — flags object literal keys that need quotes (contain
//! special characters) but don't have them, OR are quoted unnecessarily
//! (only allowed with `consistent-as-needed`).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct QuoteProps;

impl Rule for QuoteProps {
    fn id(&self) -> &'static str {
        "quote-props"
    }
    fn name(&self) -> &'static str {
        "Quote object keys properly"
    }
    fn description(&self) -> &'static str {
        "Object keys with special characters must be quoted."
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
                if node.kind() != "pair" {
                    return;
                }
                let Some(key) = node.child_by_field_name("key") else {
                    return;
                };
                if key.kind() != "property_identifier" {
                    return;
                }
                let Ok(text) = key.utf8_text(source.as_bytes()) else {
                    return;
                };
                if needs_quoting(text) {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "quote-props".into(),
                        severity: Severity::Minor,
                        message: format!(
                            "Quote the key `{}` (special characters in identifier).",
                            text
                        ),
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

fn needs_quoting(name: &str) -> bool {
    // Keys that contain hyphens, spaces, or other non-identifier characters.
    name.chars()
        .any(|c| !c.is_alphanumeric() && c != '_' && c != '$')
}
