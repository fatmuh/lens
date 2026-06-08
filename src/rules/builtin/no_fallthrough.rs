//! `no-fallthrough` — flags `switch` cases that don't end with break/return/throw.

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoFallthrough;

impl Rule for NoFallthrough {
    fn id(&self) -> &'static str {
        "no-fallthrough"
    }
    fn name(&self) -> &'static str {
        "No `switch` fall-through"
    }
    fn description(&self) -> &'static str {
        "Each `switch` case should end with `break`/`return`/`throw` to avoid accidental fall-through."
    }
    fn default_severity(&self) -> Severity {
        Severity::Critical
    }
    fn languages(&self) -> &[Language] {
        &[
            Language::TypeScript,
            Language::Tsx,
            Language::JavaScript,
            Language::Jsx,
            Language::Dart,
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "switch_body" {
                    return;
                }
                let cases = collect_cases(node);
                for i in 0..cases.len().saturating_sub(1) {
                    if !case_terminates(cases[i]) && !case_is_empty(cases[i]) {
                        // Don't flag if previous case was also empty (cascading empty).
                        let start = cases[i].start_position();
                        let end = cases[i].end_position();
                        issues.push(Issue {
                            rule_id: "no-fallthrough".into(),
                            severity: Severity::Critical,
                            message: "Switch case falls through to the next case. Add `break`."
                                .into(),
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

fn collect_cases(body: Node) -> Vec<Node> {
    let mut out = Vec::new();
    let mut cursor = body.walk();
    for c in body.children(&mut cursor) {
        if c.kind() == "switch_case" || c.kind() == "switch_default" {
            out.push(c);
        }
    }
    out
}

fn case_terminates(case: Node) -> bool {
    let mut cursor = case.walk();
    for c in case.children(&mut cursor) {
        // Skip the case label (e.g. "case 1:") and look at the statements.
        if c.kind() == "case" || c.kind() == "default" || c.kind() == ":" {
            continue;
        }
        if matches!(
            c.kind(),
            "break_statement" | "return_statement" | "throw_statement" | "continue_statement"
        ) {
            return true;
        }
    }
    false
}

fn case_is_empty(case: Node) -> bool {
    let mut count = 0;
    let mut cursor = case.walk();
    for c in case.children(&mut cursor) {
        if !matches!(c.kind(), "case" | "default" | ":") {
            count += 1;
        }
    }
    count == 0
}
