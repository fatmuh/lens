//! `no-throw-literal` — flags `throw "string"` or `throw 42`.
//! Use `throw new Error(...)` instead so the runtime gets a proper stack.

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoThrowLiteral;

impl Rule for NoThrowLiteral {
    fn id(&self) -> &'static str {
        "no-throw-literal"
    }
    fn name(&self) -> &'static str {
        "No `throw <literal>`"
    }
    fn description(&self) -> &'static str {
        "Only `throw` an `Error` object. String/number/boolean literals don't carry stack traces."
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
                if node.kind() != "throw_statement" {
                    return;
                }
                // throw_statement → throw + expression
                // We want the *argument* to throw, which is the first non-throw child.
                let arg = node
                    .children(&mut node.walk())
                    .find(|c| c.kind() != "throw");
                let Some(arg) = arg else {
                    return;
                };
                let kind = arg.kind();
                let is_literal = matches!(
                    kind,
                    "string"
                        | "number"
                        | "true"
                        | "false"
                        | "null"
                        | "template_string"
                        | "unary_expression"
                        | "regex"
                );
                // `throw undefined` and `throw null` are also bad form.
                let is_undefined = kind == "undefined";
                if is_literal || is_undefined {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-throw-literal".into(),
                        severity: Severity::Major,
                        message: "Only `throw` an `Error` object.".into(),
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
