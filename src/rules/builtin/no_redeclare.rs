//! `no-redeclare` — flags `var`/`let`/`const` declarations that share a
//! name with another declaration in the SAME top-level scope. Block-scoped
//! shadowing is allowed (this is a heuristic, not full scope analysis).

use std::collections::HashSet;

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoRedeclare;

impl Rule for NoRedeclare {
    fn id(&self) -> &'static str {
        "no-redeclare"
    }
    fn name(&self) -> &'static str {
        "No redeclaration"
    }
    fn description(&self) -> &'static str {
        "Don't redeclare a top-level variable. (Block-scoped shadowing allowed.)"
    }
    fn default_severity(&self) -> Severity {
        Severity::Critical
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
            check_top_level(tree.root_node(), file, source, &mut issues);
        });
        issues
    }
}

fn check_top_level(root: Node, file: &FileAnalysis, source: &str, issues: &mut Vec<Issue>) {
    // Only check direct children of the program root (top-level declarations).
    let mut seen: HashSet<String> = HashSet::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if matches!(child.kind(), "variable_declaration" | "lexical_declaration") {
            // Extract declarator names from this declaration.
            let mut dc = child.walk();
            for d in child.children(&mut dc) {
                if d.kind() == "variable_declarator" {
                    if let Some(name) = d.child_by_field_name("name") {
                        if let Ok(text) = name.utf8_text(source.as_bytes()) {
                            if !seen.insert(text.to_string()) {
                                let start = d.start_position();
                                let line = start.row as u32 + 1;
                                issues.push(Issue {
                                    rule_id: "no-redeclare".into(),
                                    severity: Severity::Critical,
                                    message: format!(
                                        "`{}` is already declared at the top level.",
                                        text
                                    ),
                                    file: file.path.clone(),
                                    start_line: line,
                                    end_line: line,
                                    start_column: start.column as u32,
                                    end_column: (start.column + text.len()) as u32,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}
