//! `no-dupe-keys` — flags object literals with duplicate property keys.

use std::collections::HashSet;

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoDupeKeys;

impl Rule for NoDupeKeys {
    fn id(&self) -> &'static str { "no-dupe-keys" }
    fn name(&self) -> &'static str { "No duplicate object keys" }
    fn description(&self) -> &'static str {
        "Object literals should not have duplicate keys. Only the last one wins."
    }
    fn default_severity(&self) -> Severity { Severity::Critical }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "object" { return; }
                let mut seen: HashSet<String> = HashSet::new();
                let mut cursor = node.walk();
                for c in node.children(&mut cursor) {
                    if matches!(c.kind(), "pair" | "shorthand_property_identifier_pattern" | "spread_element" | "method_definition") {
                        let key = extract_key(c, source);
                        if let Some(k) = key {
                            if !seen.insert(k.clone()) {
                                let start = c.start_position();
                                let end = c.end_position();
                                issues.push(Issue {
                                    rule_id: "no-dupe-keys".into(),
                                    severity: Severity::Critical,
                                    message: format!("Duplicate key `{}` in object literal.", k),
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

fn extract_key(pair: Node, source: &str) -> Option<String> {
    match pair.kind() {
        "pair" => {
            if let Some(key) = pair.child_by_field_name("key") {
                return key.utf8_text(source.as_bytes()).ok().map(|s| s.trim_matches(|c: char| c == '"' || c == '\'' || c == '`').to_string());
            }
            None
        }
        "shorthand_property_identifier_pattern" => {
            pair.utf8_text(source.as_bytes()).ok().map(String::from)
        }
        _ => None,
    }
}
