//! `prefer-const` — flags `let` declarations that are never reassigned.

use std::collections::HashSet;

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct PreferConst;

impl Rule for PreferConst {
    fn id(&self) -> &'static str { "prefer-const" }
    fn name(&self) -> &'static str { "Prefer `const`" }
    fn description(&self) -> &'static str {
        "Use `const` for variables that are never reassigned after declaration."
    }
    fn default_severity(&self) -> Severity { Severity::Minor }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            let root = tree.root_node();
            // For each `let` declaration, check whether the binding name is
            // reassigned anywhere later in its scope.
            let mut lets: Vec<(String, Node)> = Vec::new();
            let mut reassigned: HashSet<String> = HashSet::new();

            crate::analyzer::parser::visit_descendants(root, |node| {
                if node.kind() == "variable_declaration" {
                    if let Some(first) = node.child(0) {
                        if let Ok(text) = first.utf8_text(source.as_bytes()) {
                            if text == "let" {
                                if let Some(name) = extract_declaration_name(node, source) {
                                    lets.push((name, node));
                                }
                            }
                        }
                    }
                }
                if node.kind() == "assignment_expression" {
                    if let Some(left) = node.child_by_field_name("left") {
                        if let Ok(text) = left.utf8_text(source.as_bytes()) {
                            reassigned.insert(text.to_string());
                        }
                    }
                }
                // Update expressions: `i++`, `--i`
                if node.kind() == "update_expression" {
                    if let Some(arg) = node.child_by_field_name("argument") {
                        if let Ok(text) = arg.utf8_text(source.as_bytes()) {
                            reassigned.insert(text.to_string());
                        }
                    }
                }
            });

            for (name, decl_node) in lets {
                if !reassigned.contains(&name) {
                    let start = decl_node.start_position();
                    issues.push(Issue {
                        rule_id: "prefer-const".into(),
                        severity: Severity::Minor,
                        message: format!("`{}` is never reassigned; use `const` instead of `let`.", name),
                        file: file.path.clone(),
                        start_line: start.row as u32 + 1,
                        end_line: start.row as u32 + 1,
                        start_column: start.column as u32,
                        end_column: (start.column + 3) as u32,
                    });
                }
            }
        });
        issues
    }
}

fn extract_declaration_name(decl: Node, source: &str) -> Option<String> {
    // variable_declarator: name = value
    for i in 0..decl.child_count() {
        if let Some(child) = decl.child(i) {
            if child.kind() == "variable_declarator" {
                if let Some(name) = child.child_by_field_name("name") {
                    if let Ok(text) = name.utf8_text(source.as_bytes()) {
                        return Some(text.to_string());
                    }
                }
            }
        }
    }
    None
}
