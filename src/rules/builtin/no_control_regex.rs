//! `no-control-regex` — flags regex literals containing ASCII control characters
//! (which almost always indicates a mistake).

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoControlRegex;

impl Rule for NoControlRegex {
    fn id(&self) -> &'static str { "no-control-regex" }
    fn name(&self) -> &'static str { "No control characters in regex" }
    fn description(&self) -> &'static str {
        "Regex literals shouldn't contain ASCII control characters."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "regex_pattern" { return; }
                if let Ok(text) = node.utf8_text(source.as_bytes()) {
                    // `regex_pattern` text contains only the pattern (no / or flags).
                    let inner = text;
                    let mut has_control = false;
                    // Literal control char.
                    if inner.chars().any(|c| c.is_ascii_control()) {
                        has_control = true;
                    }
                    // Hex escape: \xNN where NN is 00-1F or 7F.
                    let bytes = inner.as_bytes();
                    let mut i = 0;
                    while i + 3 < bytes.len() {
                        if bytes[i] == b'\\' && bytes[i+1] == b'x' {
                            if let Some(hex) = std::str::from_utf8(&bytes[i+2..i+4]).ok()
                                .and_then(|s| u8::from_str_radix(s, 16).ok())
                            {
                                if hex < 0x20 || hex == 0x7F {
                                    has_control = true;
                                }
                            }
                        }
                        i += 1;
                    }
                    if has_control {
                        let start = node.start_position();
                        let end = node.end_position();
                        issues.push(Issue {
                            rule_id: "no-control-regex".into(),
                            severity: Severity::Major,
                            message: "Avoid ASCII control characters in regex.".into(),
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
