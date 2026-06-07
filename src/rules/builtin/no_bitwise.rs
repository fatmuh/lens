//! `no-bitwise` — flags `&`, `|`, `^`, `~`, `<<`, `>>`, `>>>` in regular code.
//! (Bitwise ops are a common bug; use math for masks/colors/etc.)

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoBitwise;

const OPS: &[&str] = &["&", "|", "^", "~", "<<", ">>", ">>>"];

impl Rule for NoBitwise {
    fn id(&self) -> &'static str { "no-bitwise" }
    fn name(&self) -> &'static str { "No bitwise operators" }
    fn description(&self) -> &'static str {
        "Avoid `&`/`|`/`^`/`~` in regular code. They are usually bugs."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if let Some(op) = binary_op_text(node, source) {
                    if OPS.contains(&op.as_str()) {
                        push(node, file, &mut issues, format!("Avoid bitwise operator `{}` in regular code.", op));
                    }
                }
                // Unary ~
                if node.kind() == "unary_expression" {
                    if let Ok(text) = node.utf8_text(source.as_bytes()) {
                        if text.starts_with('~') {
                            push(node, file, &mut issues, "Avoid unary `~` (bitwise NOT).".into());
                        }
                    }
                }
            });
        });
        issues
    }
}

fn binary_op_text(node: Node, source: &str) -> Option<String> {
    if !matches!(node.kind(), "binary_expression") { return None; }
    let op = node.child_by_field_name("operator")?;
    op.utf8_text(source.as_bytes()).ok().map(String::from)
}

fn push(node: Node, file: &FileAnalysis, issues: &mut Vec<Issue>, msg: String) {
    let start = node.start_position();
    let end = node.end_position();
    issues.push(Issue {
        rule_id: "no-bitwise".into(),
        severity: Severity::Major,
        message: msg,
        file: file.path.clone(),
        start_line: start.row as u32 + 1,
        end_line: end.row as u32 + 1,
        start_column: start.column as u32,
        end_column: end.column as u32,
    });
}
