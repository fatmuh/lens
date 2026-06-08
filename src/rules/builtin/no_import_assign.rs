//! `no-import-assign` — flags reassigning imported bindings.

use std::collections::HashMap;

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoImportAssign;

impl Rule for NoImportAssign {
    fn id(&self) -> &'static str {
        "no-import-assign"
    }
    fn name(&self) -> &'static str {
        "No import reassignment"
    }
    fn description(&self) -> &'static str {
        "Don't reassign imported bindings. They are read-only by convention."
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
            Language::Dart,
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            // Collect imported names.
            let mut imported: HashMap<String, Node> = HashMap::new();
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() == "import_statement" {
                    // import { a as b } from "..."
                    // we want the local name (b in this case)
                    let mut cursor = node.walk();
                    for c in node.children(&mut cursor) {
                        if c.kind() == "import_clause" {
                            let mut c2 = c.walk();
                            for cc in c.children(&mut c2) {
                                if cc.kind() == "named_imports" {
                                    let mut c3 = cc.walk();
                                    for spec in cc.children(&mut c3) {
                                        if spec.kind() == "import_specifier" {
                                            let local = spec
                                                .child_by_field_name("alias")
                                                .or_else(|| spec.child_by_field_name("name"));
                                            if let Some(l) = local {
                                                if let Ok(text) = l.utf8_text(source.as_bytes()) {
                                                    imported.insert(text.to_string(), l);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
            // Look for assignments where the LHS is an imported name.
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() == "assignment_expression" {
                    if let Some(left) = node.child_by_field_name("left") {
                        if let Ok(text) = left.utf8_text(source.as_bytes()) {
                            if imported.contains_key(text) {
                                let start = node.start_position();
                                let end = node.end_position();
                                issues.push(Issue {
                                    rule_id: "no-import-assign".into(),
                                    severity: Severity::Major,
                                    message: format!("`{}` is an import; don't reassign it.", text),
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
                // Also catch update expressions: `i++`, `--i`
                if node.kind() == "update_expression" {
                    if let Some(arg) = node.child_by_field_name("argument") {
                        if let Ok(text) = arg.utf8_text(source.as_bytes()) {
                            if imported.contains_key(text) {
                                let start = node.start_position();
                                let end = node.end_position();
                                issues.push(Issue {
                                    rule_id: "no-import-assign".into(),
                                    severity: Severity::Major,
                                    message: format!("`{}` is an import; don't mutate it.", text),
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
