//! `no-duplicate-imports` — flags importing the same module twice.

use std::collections::HashMap;

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoDuplicateImports;

impl Rule for NoDuplicateImports {
    fn id(&self) -> &'static str { "no-duplicate-imports" }
    fn name(&self) -> &'static str { "No duplicate imports" }
    fn description(&self) -> &'static str {
        "Don't import the same module more than once in a single file."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            // Collect (source, position) per import.
            let mut seen: HashMap<String, Node> = HashMap::new();
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "import_statement" { return; }
                if let Some(src) = node.child_by_field_name("source") {
                    if let Ok(text) = src.utf8_text(source.as_bytes()) {
                        let key = text.to_string();
                        if let Some(first) = seen.get(&key) {
                            let start = node.start_position();
                            let end = node.end_position();
                            issues.push(Issue {
                                rule_id: "no-duplicate-imports".into(),
                                severity: Severity::Major,
                                message: format!("`{}` is already imported at line {}.", key.trim_matches('"').trim_matches('\''), first.start_position().row as u32 + 1),
                                file: file.path.clone(),
                                start_line: start.row as u32 + 1,
                                end_line: end.row as u32 + 1,
                                start_column: start.column as u32,
                                end_column: end.column as u32,
                            });
                        } else {
                            seen.insert(key, node);
                        }
                    }
                }
            });
        });
        issues
    }
}
