//! `no-warning-comments` — flags TODO/FIXME/XXX comments without an owner.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoWarningComments;

const MARKERS: &[&str] = &["TODO", "FIXME", "XXX", "HACK"];

impl Rule for NoWarningComments {
    fn id(&self) -> &'static str { "no-warning-comments" }
    fn name(&self) -> &'static str { "No `TODO`/`FIXME` comments" }
    fn description(&self) -> &'static str {
        "`TODO`/`FIXME`/etc. comments should reference an owner (e.g. `TODO(jane):`)."
    }
    fn default_severity(&self) -> Severity { Severity::Info }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "comment" { return; }
                if let Ok(text) = node.utf8_text(source.as_bytes()) {
                    let upper = text.to_ascii_uppercase();
                    for m in MARKERS {
                        if upper.contains(m) {
                            // Check if the marker is followed by `(owner)`.
                            let has_owner = check_owner(text, *m);
                            if !has_owner {
                                let start = node.start_position();
                                let end = node.end_position();
                                issues.push(Issue {
                                    rule_id: "no-warning-comments".into(),
                                    severity: Severity::Info,
                                    message: format!("Add an owner to this `{}` comment (e.g. `TODO(name): ...`).", m),
                                    file: file.path.clone(),
                                    start_line: start.row as u32 + 1,
                                    end_line: end.row as u32 + 1,
                                    start_column: start.column as u32,
                                    end_column: end.column as u32,
                                });
                                break;
                            }
                        }
                    }
                }
            });
        });
        issues
    }
}

fn check_owner(comment: &str, marker: &str) -> bool {
    // Find marker and check if next char is `(` (owner follows).
    if let Some(idx) = comment.to_ascii_uppercase().find(marker) {
        let after = &comment[idx + marker.len()..];
        let trimmed = after.trim_start();
        return trimmed.starts_with('(');
    }
    false
}
