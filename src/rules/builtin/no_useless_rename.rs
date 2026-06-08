//! `no-useless-rename` — flags `import { x as x }` (rename to same name).

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUselessRename;

impl Rule for NoUselessRename {
    fn id(&self) -> &'static str {
        "no-useless-rename"
    }
    fn name(&self) -> &'static str {
        "No useless rename"
    }
    fn description(&self) -> &'static str {
        "Don't `import { x as x }` — the rename is identical to the original."
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
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
                if node.kind() != "import_specifier" {
                    return;
                }
                let Some(name) = node.child_by_field_name("name") else {
                    return;
                };
                let Some(alias) = node.child_by_field_name("alias") else {
                    return;
                };
                let Ok(n) = name.utf8_text(source.as_bytes()) else {
                    return;
                };
                let Ok(a) = alias.utf8_text(source.as_bytes()) else {
                    return;
                };
                if n == a {
                    let start = node.start_position();
                    let end = node.end_position();
                    issues.push(Issue {
                        rule_id: "no-useless-rename".into(),
                        severity: Severity::Minor,
                        message: format!("Useless rename `{} as {}`; just `{}`.", n, a, n),
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
