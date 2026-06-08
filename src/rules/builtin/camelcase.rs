//! `camelcase` — flags non-camelCase variable and function declarations.
//! (Disabled for SCREAMING_SNAKE_CASE constants and UPPER_CASE constants.)

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct Camelcase;

impl Rule for Camelcase {
    fn id(&self) -> &'static str {
        "camelcase"
    }
    fn name(&self) -> &'static str {
        "Use camelCase"
    }
    fn description(&self) -> &'static str {
        "Variables and functions should be camelCase (or UPPER_CASE for constants)."
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
                if node.kind() != "variable_declarator" {
                    return;
                }
                let Some(name) = node.child_by_field_name("name") else {
                    return;
                };
                if name.kind() != "identifier" {
                    return;
                }
                let Ok(text) = name.utf8_text(source.as_bytes()) else {
                    return;
                };
                // Skip already-valid: all lowercase, camelCase, or ALL_CAPS.
                if is_valid(text) {
                    return;
                }
                let start = node.start_position();
                let end = node.end_position();
                issues.push(Issue {
                    rule_id: "camelcase".into(),
                    severity: Severity::Minor,
                    message: format!("`{}` should be camelCase or UPPER_CASE.", text),
                    file: file.path.clone(),
                    start_line: start.row as u32 + 1,
                    end_line: end.row as u32 + 1,
                    start_column: start.column as u32,
                    end_column: end.column as u32,
                });
            });
        });
        issues
    }
}

fn is_valid(name: &str) -> bool {
    if name.is_empty() {
        return true;
    }
    // UPPER_CASE accepted.
    if name
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return true;
    }
    // camelCase: starts lowercase, no internal underscore (except allowed
    // leading underscore for unused params), no consecutive uppercase.
    let mut chars = name.chars().peekable();
    if !chars
        .peek()
        .map_or(false, |c| c.is_ascii_lowercase() || *c == '_')
    {
        return false;
    }
    // No snake_case.
    if name.contains('_') {
        // Allow leading underscore.
        if name.starts_with('_') {
            return !name[1..].contains('_');
        }
        return false;
    }
    true
}
