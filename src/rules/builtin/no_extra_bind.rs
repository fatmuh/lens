//! `no-extra-bind` — flags `foo.bind(this)` where `foo` doesn't use `this`.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoExtraBind;

impl Rule for NoExtraBind {
    fn id(&self) -> &'static str { "no-extra-bind" }
    fn name(&self) -> &'static str { "No extra `.bind()`" }
    fn description(&self) -> &'static str {
        "Don't `.bind(this)` a function that doesn't use `this`."
    }
    fn default_severity(&self) -> Severity { Severity::Minor }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "call_expression" { return; }
                let Some(func) = node.child_by_field_name("function") else { return; };
                if func.kind() != "member_expression" { return; };
                let Some(prop) = func.child_by_field_name("property") else { return; };
                let Ok(prop_text) = prop.utf8_text(source.as_bytes()) else { return; };
                if prop_text != "bind" { return; }
                // The bound function (object of .bind).
                let Some(bound) = func.child_by_field_name("object") else { return; };
                let Ok(bound_text) = bound.utf8_text(source.as_bytes()) else { return; };
                // Heuristic: if the bound function's source doesn't contain
                // `this` anywhere, it's an extra bind.
                if !bound_text.contains("this") {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-extra-bind".into(),
                        severity: Severity::Minor,
                        message: "`.bind(this)` is unnecessary; the function doesn't use `this`.".into(),
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
