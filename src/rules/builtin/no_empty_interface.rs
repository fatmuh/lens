//! `no-empty-interface` — flags `interface Foo {}` and `interface Foo extends Bar {}`.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoEmptyInterface;

impl Rule for NoEmptyInterface {
    fn id(&self) -> &'static str { "no-empty-interface" }
    fn name(&self) -> &'static str { "No empty interface" }
    fn description(&self) -> &'static str {
        "Empty interfaces are usually a mistake. Use a `type` alias instead."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        if !matches!(lang, Language::TypeScript | Language::Tsx) { return issues; }
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "interface_declaration" { return; }
                // Body must be present and have no members.
                let Some(body) = node.child_by_field_name("body") else { return; };
                if body.kind() != "interface_body" { return; }
                let mut real = 0;
                let mut cursor = body.walk();
                for c in body.children(&mut cursor) {
                    if !matches!(c.kind(), "{" | "}" | "comment") {
                        real += 1;
                    }
                }
                // Also check for `extends` clause — extends makes the interface non-empty.
                let has_extends = node.children(&mut node.walk())
                    .any(|c| c.kind() == "extends_type_clause" || c.kind() == "extends_clause");
                if real == 0 && !has_extends {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-empty-interface".into(),
                        severity: Severity::Major,
                        message: "Empty interface; use a `type` alias instead.".into(),
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
