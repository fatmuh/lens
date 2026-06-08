//! `no-sparse-arrays` — flags array literals with holes (`[1, , 3]`).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoSparseArrays;

impl Rule for NoSparseArrays {
    fn id(&self) -> &'static str {
        "no-sparse-arrays"
    }
    fn name(&self) -> &'static str {
        "No sparse arrays"
    }
    fn description(&self) -> &'static str {
        "Don't create sparse arrays (`[1, , 3]`); use `undefined` explicitly."
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
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
                if node.kind() != "array" {
                    return;
                }
                // Walk children, looking for a `,` that's followed by `,` or `]`.
                let mut prev_comma = false;
                let mut cursor = node.walk();
                for c in node.children(&mut cursor) {
                    if c.kind() == "," {
                        if prev_comma {
                            // Two commas in a row → hole.
                            let start = node.start_position();
                            let end = node.end_position();
                            issues.push(Issue {
                                rule_id: "no-sparse-arrays".into(),
                                severity: Severity::Major,
                                message: "Sparse array (consecutive commas). Use `undefined`."
                                    .into(),
                                file: file.path.clone(),
                                start_line: start.row as u32 + 1,
                                end_line: end.row as u32 + 1,
                                start_column: start.column as u32,
                                end_column: end.column as u32,
                            });
                            return;
                        }
                        prev_comma = true;
                    } else if !matches!(c.kind(), "[" | "]") {
                        prev_comma = false;
                    }
                }
            });
        });
        issues
    }
}
