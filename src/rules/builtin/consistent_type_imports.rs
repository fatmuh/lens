//! `consistent-type-imports` — flags `import { Foo } from "bar"` where
//! `Foo` is a type. Should be `import type { Foo } from "bar"`.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct ConsistentTypeImports;

impl Rule for ConsistentTypeImports {
    fn id(&self) -> &'static str {
        "consistent-type-imports"
    }
    fn name(&self) -> &'static str {
        "Use `import type`"
    }
    fn description(&self) -> &'static str {
        "Use `import type { Foo }` for type-only imports. Helps bundlers strip types."
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
            // First: build a set of names used as values (vs only as types).
            let mut used_as_value: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                // Track identifier usage outside of import statements and
                // type positions. We use a rough heuristic: any reference
                // outside an import_statement or type_annotation is a use.
                if node.kind() == "import_statement"
                    || node.kind() == "type_annotation"
                    || node.kind() == "type_identifier"
                    || node.kind() == "predefined_type"
                    || node.kind() == "generic_type"
                {
                    return;
                }
                if matches!(node.kind(), "identifier" | "property_identifier") {
                    if let Some(parent) = node.parent() {
                        // Don't count the LHS of an import_clause.
                        if parent.kind() == "import_clause" {
                            return;
                        }
                    }
                    if let Ok(text) = node.utf8_text(source.as_bytes()) {
                        used_as_value.insert(text.to_string());
                    }
                }
            });

            // Now: walk import statements, find each imported name, and if
            // it's NOT used as a value, suggest `import type`.
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "import_statement" {
                    return;
                }
                if let Some(text) = node.child_by_field_name("type") {
                    let _ = text; // already has `type` keyword
                    return;
                }
                // Walk the import clause's named imports.
                let mut cursor = node.walk();
                for c in node.children(&mut cursor) {
                    if c.kind() == "import_clause" {
                        walk_import_clause(c, file, source, &used_as_value, &mut issues);
                    }
                }
            });
        });
        issues
    }
}

fn walk_import_clause(
    clause: Node,
    file: &FileAnalysis,
    source: &str,
    used_as_value: &std::collections::HashSet<String>,
    issues: &mut Vec<Issue>,
) {
    let mut cursor = clause.walk();
    for c in clause.children(&mut cursor) {
        if c.kind() == "named_imports" {
            let mut nc = c.walk();
            for spec in c.children(&mut nc) {
                if spec.kind() == "import_specifier" {
                    if let Some(name) = spec.child_by_field_name("name") {
                        if let Ok(text) = name.utf8_text(source.as_bytes()) {
                            if !used_as_value.contains(text) {
                                let start = spec.start_position();
                                let end = spec.end_position();
                                issues.push(Issue {
                                    rule_id: "consistent-type-imports".into(),
                                    severity: Severity::Minor,
                                    message: format!(
                                        "`{}` is only used as a type; use `import type`.",
                                        text
                                    ),
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
            }
        }
    }
}
