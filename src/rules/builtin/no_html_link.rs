//! `no-html-link` — flags `dangerouslySetInnerHTML` in JSX/TSX. Common
//! XSS vector — prefer a library that escapes by default.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoHtmlLink;

impl Rule for NoHtmlLink {
    fn id(&self) -> &'static str {
        "no-html-link"
    }
    fn name(&self) -> &'static str {
        "No `dangerouslySetInnerHTML`"
    }
    fn description(&self) -> &'static str {
        "Avoid `dangerouslySetInnerHTML`. It's an XSS vector unless the value is trusted and escaped."
    }
    fn default_severity(&self) -> Severity {
        Severity::Critical
    }
    fn languages(&self) -> &[Language] {
        &[Language::Tsx, Language::Jsx]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        if !matches!(lang, Language::Tsx | Language::Jsx) {
            return issues;
        }
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                // JSX attributes have their name as a child node. Look for
                // the attribute whose name is "dangerouslySetInnerHTML".
                let name_text = match jsx_attribute_name_text(node, source) {
                    Some(t) => t,
                    None => return,
                };
                if name_text == "dangerouslySetInnerHTML" {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-html-link".into(),
                        severity: Severity::Critical,
                        message: "Avoid `dangerouslySetInnerHTML`; it's an XSS vector.".into(),
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

fn jsx_attribute_name_text(node: Node, source: &str) -> Option<String> {
    // tree-sitter-typescript JSX: jsx_attribute with a "name" child
    // (property_identifier or jsx_identifier).
    if node.kind() != "jsx_attribute" {
        return None;
    }
    // The attribute is the parent of its name; the name is one of the
    // first children.
    let mut cursor = node.walk();
    for c in node.children(&mut cursor) {
        if matches!(c.kind(), "property_identifier" | "jsx_identifier") {
            return c.utf8_text(source.as_bytes()).ok().map(String::from);
        }
    }
    // Fallback: scan the source text of the attribute.
    let Ok(text) = node.utf8_text(source.as_bytes()) else {
        return None;
    };
    if text.contains("dangerouslySetInnerHTML") {
        Some("dangerouslySetInnerHTML".to_string())
    } else {
        None
    }
}
