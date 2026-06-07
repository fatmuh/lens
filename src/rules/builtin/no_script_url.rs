//! `no-script-url` — flags `javascript:` URLs which can execute code.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoScriptUrl;

impl Rule for NoScriptUrl {
    fn id(&self) -> &'static str { "no-script-url" }
    fn name(&self) -> &'static str { "No `javascript:` URLs" }
    fn description(&self) -> &'static str {
        "Avoid `javascript:` URLs. They execute arbitrary code when clicked."
    }
    fn default_severity(&self) -> Severity { Severity::Critical }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() == "string" || node.kind() == "template_string" {
                    if let Ok(text) = node.utf8_text(source.as_bytes()) {
                        // Strip surrounding quotes/backticks.
                        let inner = text.trim_matches(|c: char| c == '"' || c == '\'' || c == '`');
                        let lower = inner.to_ascii_lowercase();
                        if lower.starts_with("javascript:") {
                            let start = node.start_position();
                            let end = node.end_position();
                            issues.push(Issue {
                                rule_id: "no-script-url".into(),
                                severity: Severity::Critical,
                                message: "Avoid `javascript:` URLs; they execute arbitrary code.".into(),
                                file: file.path.clone(),
                                start_line: start.row as u32 + 1,
                                end_line: end.row as u32 + 1,
                                start_column: start.column as u32,
                                end_column: end.column as u32,
                            });
                        }
                    }
                }
            });
        });
        issues
    }
}
