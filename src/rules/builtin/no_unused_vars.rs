//! `no-unused-vars` — flags declared variables and parameters that are
//! never referenced.
//!
//! This is a per-function check, not a full scope analysis. It walks each
//! function and looks for declarations whose names never appear in any
//! descendant identifier node.

use std::collections::HashSet;

use tree_sitter::Node;

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

pub struct NoUnusedVars;

impl Rule for NoUnusedVars {
    fn id(&self) -> &'static str {
        "no-unused-vars"
    }
    fn name(&self) -> &'static str {
        "No unused variables"
    }
    fn description(&self) -> &'static str {
        "Remove variables and parameters that are declared but never used."
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
            crate::analyzer::parser::visit_descendants(tree.root_node(), |func| {
                if !matches!(
                    func.kind(),
                    "function_declaration"
                        | "function"
                        | "method_definition"
                        | "arrow_function"
                        | "generator_function_declaration"
                ) {
                    return;
                }
                check_function(func, file, source, &mut issues);
            });
        });
        issues
    }
}

fn check_function(func: Node, file: &FileAnalysis, source: &str, issues: &mut Vec<Issue>) {
    // 1. Collect all *declarations* within this function: parameters + let/const/var.
    let mut decls: Vec<(String, Node)> = Vec::new();
    crate::analyzer::parser::visit_descendants(func, |node| {
        // Parameters
        if node.kind() == "formal_parameters" {
            for i in 0..node.child_count() {
                if let Some(p) = node.child(i) {
                    if let Some(name) = parameter_name(p, source) {
                        if let Ok(text) = p.utf8_text(source.as_bytes()) {
                            if !text.starts_with('_') {
                                decls.push((name, p));
                            }
                        }
                    }
                }
            }
        }
        // let/const/var declarations
        if node.kind() == "variable_declarator" {
            if let Some(name) = node.child_by_field_name("name") {
                if let Ok(text) = name.utf8_text(source.as_bytes()) {
                    if !text.starts_with('_') {
                        decls.push((text.to_string(), name));
                    }
                }
            }
        }
    });

    // 2. Collect all *identifiers* referenced within the function body,
    //    excluding identifiers that are at the declaration position
    //    (i.e. the parameter name or variable declarator name itself).
    let mut used: HashSet<String> = HashSet::new();
    crate::analyzer::parser::visit_descendants(func, |node| {
        if matches!(
            node.kind(),
            "identifier" | "property_identifier" | "shorthand_property_identifier_pattern"
        ) {
            if is_in_declaration_position(node) {
                return;
            }
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                used.insert(text.to_string());
            }
        }
    });

    // 3. Report declarations whose name is never used.
    for (name, decl_node) in decls {
        if !used.contains(&name) {
            let start = decl_node.start_position();
            let end = decl_node.end_position();
            issues.push(Issue {
                rule_id: "no-unused-vars".into(),
                severity: Severity::Major,
                message: format!("`{}` is declared but never used.", name),
                file: file.path.clone(),
                start_line: start.row as u32 + 1,
                end_line: end.row as u32 + 1,
                start_column: start.column as u32,
                end_column: end.column as u32,
            });
        }
    }
}

fn parameter_name(p: Node, source: &str) -> Option<String> {
    match p.kind() {
        "identifier" => p.utf8_text(source.as_bytes()).ok().map(String::from),
        "required_parameter" | "optional_parameter" => {
            // pattern → name
            p.child_by_field_name("pattern")
                .and_then(|pat| match pat.kind() {
                    "identifier" => pat.utf8_text(source.as_bytes()).ok().map(String::from),
                    _ => None,
                })
        }
        _ => None,
    }
}

/// True if the identifier is the *name* of a parameter or variable
/// declarator (i.e. the LHS of a declaration), as opposed to a use.
fn is_in_declaration_position(node: Node) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        match parent.kind() {
            "formal_parameters" => return true,
            "variable_declarator" => {
                // Only the "name" field of a variable_declarator is the
                // declaration; the value side is a use.
                if let Some(name) = parent.child_by_field_name("name") {
                    if name.start_position() == node.start_position()
                        && name.end_position() == node.end_position()
                    {
                        return true;
                    }
                }
                return false;
            }
            _ => current = parent,
        }
    }
    false
}
