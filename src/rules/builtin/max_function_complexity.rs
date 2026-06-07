//! `max-function-complexity` — flags functions whose cognitive complexity
//! exceeds a threshold (default 15, matching SonarJS S3776). Threshold
//! comes from `[rules.max_function_complexity]` in quality-gate.toml.

use tree_sitter::Node;

use crate::analyzer::cognitive::cognitive_complexity;
use crate::analyzer::FileAnalysis;
use crate::analyzer::parser::{visit_descendants, with_parser};
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct MaxFunctionComplexity {
    pub threshold: u32,
}

impl Default for MaxFunctionComplexity {
    fn default() -> Self { Self { threshold: 15 } }
}

impl MaxFunctionComplexity {
    pub fn with_threshold(threshold: u32) -> Self { Self { threshold } }
}

impl Rule for MaxFunctionComplexity {
    fn id(&self) -> &'static str { "max-function-complexity" }
    fn name(&self) -> &'static str { "Function too complex" }
    fn description(&self) -> &'static str {
        "Cognitive complexity > threshold makes functions hard to understand. Refactor into smaller pieces."
    }
    fn default_severity(&self) -> Severity { Severity::Major }
    fn languages(&self) -> &[Language] { &[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx] }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = file.language else { return issues; };
        let _ = with_parser(lang, source, |tree| {
            visit_descendants(tree.root_node(), |node| {
                if !is_function(node) { return; }
                let name = node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(String::from);
                let cc_name = name.as_deref();
                let result = cognitive_complexity(node, source, cc_name);
                if result.cc > self.threshold {
                    let func_name = name.as_deref().unwrap_or("<anonymous>");
                    issues.push(Issue {
                        rule_id: "max-function-complexity".into(),
                        severity: Severity::Major,
                        message: format!(
                            "Function `{}` has cognitive complexity {} (max {}).",
                            func_name, result.cc, self.threshold
                        ),
                        file: file.path.clone(),
                        start_line: result.start_line,
                        end_line: result.end_line,
                        start_column: 0,
                        end_column: 0,
                    });
                }
            });
        });
        issues
    }
}

fn is_function(node: Node) -> bool {
    matches!(node.kind(),
        "function_declaration"
        | "function"
        | "method_definition"
        | "function_expression"
        | "arrow_function"
    )
}
