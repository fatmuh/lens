//! `no-prototype-builtins` — flags `obj.hasOwnProperty(...)`, `obj.isPrototypeOf(...)`,
//! etc. Direct prototype access is unsafe (obj might have its own
//! `hasOwnProperty` property). Use `Object.prototype.hasOwnProperty.call(obj, ...)`.
//! Common in SonarQube's `javascript:S2870`.

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoPrototypeBuiltins;

const FORBIDDEN: &[&str] = &["hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable"];

impl Rule for NoPrototypeBuiltins {
    fn id(&self) -> &'static str {
        "no-prototype-builtins"
    }
    fn name(&self) -> &'static str {
        "No direct prototype builtins"
    }
    fn description(&self) -> &'static str {
        "Use `Object.prototype.hasOwnProperty.call(obj, key)` instead of `obj.hasOwnProperty(key)`."
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
        ]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else {
            return issues;
        };
        crate::analyzer::parser::with_parser(lang, source, |tree| {
            crate::analyzer::parser::visit_descendants(tree.root_node(), |node| {
                if node.kind() != "call_expression" {
                    return;
                }
                let Some(func) = node.child_by_field_name("function") else {
                    return;
                };
                if func.kind() != "member_expression" {
                    return;
                }
                let Some(prop) = func.child_by_field_name("property") else {
                    return;
                };
                let Ok(prop_text) = prop.utf8_text(source.as_bytes()) else {
                    return;
                };
                if !FORBIDDEN.contains(&prop_text) {
                    return;
                }
                let start = node.start_position();
                let end = node.end_position();
                issues.push(Issue {
                    rule_id: "no-prototype-builtins".into(),
                    severity: Severity::Critical,
                    message: format!(
                        "Use `Object.prototype.{}.call(...)` instead of direct `{}.`",
                        prop_text, prop_text
                    ),
                    file: file.path.clone(),
                    start_line: start.row as u32 + 1,
                    end_line: end.row as u32 + 1,
                    start_column: start.column as u32,
                    end_column: end.column as u32,
                });
            });
        });
        issues
    }
}
