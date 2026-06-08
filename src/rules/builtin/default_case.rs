//! `default-case` — flags `switch` statements that have no `default` case.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct DefaultCase;

impl Rule for DefaultCase {
    fn id(&self) -> &'static str {
        "default-case"
    }
    fn name(&self) -> &'static str {
        "Require `default` in `switch`"
    }
    fn description(&self) -> &'static str {
        "`switch` statements should have a `default` case to handle unexpected values."
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
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "switch_statement" {
                    return;
                }
                // Look for a default case anywhere in the body.
                let Some(body) = node.child_by_field_name("body") else {
                    return;
                };
                let has_default = has_descendant_kind(body, "switch_default");
                if !has_default {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "default-case".into(),
                        severity: Severity::Major,
                        message: "Add a `default` case to this `switch`.".into(),
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

fn has_descendant_kind(root: Node, kind: &str) -> bool {
    let mut found = false;
    crate::analyzer::parser::visit_descendants(root, |n| {
        if n.kind() == kind {
            found = true;
        }
    });
    found
}
