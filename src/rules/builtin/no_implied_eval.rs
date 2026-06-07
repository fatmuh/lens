//! `no-implied-eval` — flags `setTimeout("code", n)` and `setInterval(...)`
//! with a string argument. The string is implicitly `eval()`d.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoImpliedEval;

impl Rule for NoImpliedEval {
    fn id(&self) -> &'static str { "no-implied-eval" }
    fn name(&self) -> &'static str { "No implied `eval`" }
    fn description(&self) -> &'static str {
        "Avoid `setTimeout(\"code\", n)` and `setInterval(\"code\", n)`. The string is `eval()`d."
    }
    fn default_severity(&self) -> Severity { Severity::Critical }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "call_expression" { return; }
                let Some(func) = node.child_by_field_name("function") else { return; };
                if func.kind() != "identifier" { return; }
                let Ok(name) = func.utf8_text(source.as_bytes()) else { return; };
                if !matches!(name, "setTimeout" | "setInterval") { return; }
                // First argument must be a string literal.
                let mut cursor = node.walk();
                let args: Vec<Node> = node.children(&mut cursor)
                    .filter(|c| c.kind() == "arguments")
                    .flat_map(|a| {
                        let mut c = a.walk();
                        a.children(&mut c).collect::<Vec<_>>()
                    })
                    .collect();
                for arg in &args {
                    if matches!(arg.kind(), "string" | "template_string") {
                        let start = node.start_position();
                        let end = node.end_position();
                        issues.push(Issue {
                            rule_id: "no-implied-eval".into(),
                            severity: Severity::Critical,
                            message: format!("`{}` with a string argument is an implicit `eval`.", name),
                            file: file.path.clone(),
                            start_line: start.row as u32 + 1,
                            end_line: end.row as u32 + 1,
                            start_column: start.column as u32,
                            end_column: end.column as u32,
                        });
                        return;
                    }
                }
            });
        });
        issues
    }
}
