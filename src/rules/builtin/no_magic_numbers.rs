//! `no-magic-numbers` — flags numeric literals in expressions (other than
//! common ones like 0, 1, 2, -1, 100, 1000 and those below the configured
//! `min_value`).
//!
//! This is a heuristic rule — it doesn't track constants vs variables. The
//! goal is to flag obvious magic numbers, not all of them.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoMagicNumbers {
    pub min_value: u32,
}

impl Default for NoMagicNumbers {
    fn default() -> Self { Self { min_value: 3 } }
}

impl NoMagicNumbers {
    pub fn with_min_value(min_value: u32) -> Self { Self { min_value } }
}

const ALLOWED: &[i64] = &[-1, 0, 1, 2, 10, 100, 1000];

impl Rule for NoMagicNumbers {
    fn id(&self) -> &'static str { "no-magic-numbers" }
    fn name(&self) -> &'static str { "No magic numbers" }
    fn description(&self) -> &'static str {
        "Extract numeric literals into named constants for readability."
    }
    fn default_severity(&self) -> Severity { Severity::Info }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "number" { return; }
                // Skip numbers in: const/let/enum initializers (those are
                // already named), and small expressions like `arr[0]`.
                if let Some(parent) = node.parent() {
                    // Skip if part of a variable_declarator's value (already named)
                    if parent.kind() == "variable_declarator" { return; }
                    // Skip if part of an enum body
                    let mut p = parent;
                    while let Some(pp) = p.parent() {
                        if pp.kind() == "enum_declaration" { return; }
                        p = pp;
                    }
                }

                let text = match node.utf8_text(source.as_bytes()) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                // Strip numeric separators and parse
                let cleaned = text.replace('_', "");
                let n: i64 = match cleaned.parse() {
                    Ok(v) => v,
                    Err(_) => return, // float, hex, etc. — skip
                };
                if ALLOWED.contains(&n) { return; }
                if n.abs() < self.min_value as i64 { return; }
                let start = node.start_position();
                let end = node.end_position();
                issues.push(Issue {
                    rule_id: "no-magic-numbers".into(),
                    severity: Severity::Info,
                    message: format!("Extract magic number `{}` into a named constant.", text),
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
