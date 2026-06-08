//! `no-misused-new` — flags `new` on an interface (TypeScript). Interfaces
//! describe shape, not constructors.

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoMisusedNew;

impl Rule for NoMisusedNew {
    fn id(&self) -> &'static str {
        "no-misused-new"
    }
    fn name(&self) -> &'static str {
        "No `new` on interface"
    }
    fn description(&self) -> &'static str {
        "Don't use `new` on an interface; use a class."
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
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
            // Collect interface names.
            let mut interfaces: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() == "interface_declaration" {
                    if let Some(name) = node.child_by_field_name("name") {
                        if let Ok(text) = name.utf8_text(source.as_bytes()) {
                            interfaces.insert(text.to_string());
                        }
                    }
                }
            });
            // Flag any `new InterfaceName` where InterfaceName is an interface.
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "new_expression" {
                    return;
                }
                let Some(ctor) = node.child_by_field_name("constructor") else {
                    return;
                };
                if ctor.kind() != "identifier" {
                    return;
                }
                if let Ok(name) = ctor.utf8_text(source.as_bytes()) {
                    if interfaces.contains(name) {
                        let start = node.start_position();
                        let end = node.end_position();
                        issues.push(Issue {
                            rule_id: "no-misused-new".into(),
                            severity: Severity::Major,
                            message: format!(
                                "Don't use `new` on the interface `{}`; use a class.",
                                name
                            ),
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
