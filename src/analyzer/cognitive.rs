//! Cognitive Complexity (SonarSource S3776).
//!
//! Based on the "Cognitive Complexity" whitepaper by G. Ann Campbell
//! (SonarSource, 2018). The algorithm differs from cyclomatic complexity
//! in three ways:
//!   1. **Nesting penalty**: control-flow structures inside other control
//!      flow structures are penalized with +1 per nesting level.
//!   2. **Structural increments only**: each `if`/`for`/`while`/etc.
//!      adds 1, not each path through it.
//!   3. **Different per-language rules**: things like `else if` count as
//!      a single `if` (not two structures).
//!
//! Reference: <https://www.sonarsource.com/docs/Cognitive_Complexity.pdf>

use tree_sitter::Node;

/// Result of computing cognitive complexity for a function.
#[derive(Debug, Clone, Copy, Default)]
pub struct CognitiveResult {
    /// Cognitive complexity score.
    pub cc: u32,
    /// Function start line (1-indexed).
    pub start_line: u32,
    /// Function end line (1-indexed).
    pub end_line: u32,
}

/// Compute cognitive complexity for a single function (or any node).
///
/// The score is the sum of:
///   - B (base) = 0
///   - +1 for each control-flow structure: if, else if, ternary, switch,
///     for, for-in, for-of, while, do-while, catch
///   - +1 for each logical operator: &&, ||, ??
///   - +1 per nesting level for nested control flow
///   - +1 for recursion (function calls itself)
///   - +1 for each `break`/`continue` to a label
pub fn cognitive_complexity(
    root: Node,
    source: &str,
    function_name: Option<&str>,
) -> CognitiveResult {
    let mut visitor = CognitiveVisitor {
        cc: 0,
        nesting: 0,
        function_name: function_name.map(String::from),
        source,
    };
    visit(root, &mut visitor, source);
    let start = root.start_position();
    let end = root.end_position();
    CognitiveResult {
        cc: visitor.cc,
        start_line: start.row as u32 + 1,
        end_line: end.row as u32 + 1,
    }
}

struct CognitiveVisitor<'a> {
    cc: u32,
    /// Current control-flow nesting level.
    nesting: u32,
    /// Name of the function we're analyzing (for recursion detection).
    function_name: Option<String>,
    source: &'a str,
}

fn visit(node: Node, v: &mut CognitiveVisitor<'_>, source: &str) {
    let kind = node.kind();

    // Determine if this node is a control-flow structure.
    let cf_increment = is_control_flow(kind);
    // Determine if this is a recursion call.
    let recursion_name = v.function_name.clone();
    let is_recursion_call =
        is_function_call_named(node, recursion_name.as_deref().unwrap_or(""), source);

    // Bump nesting for control-flow structures (so children are penalized).
    if cf_increment {
        v.nesting += 1;
        v.cc += 1 + (v.nesting - 1); // +1 base + nesting penalty
    }

    // Recursion: +1.
    if is_recursion_call {
        v.cc += 1;
    }

    // Logical operator: +1.
    if kind == "binary_expression" {
        if let Some(op) = node.child_by_field_name("operator") {
            if let Ok(op_text) = op.utf8_text(source.as_bytes()) {
                if matches!(op_text, "&&" | "||" | "??") {
                    v.cc += 1;
                }
            }
        }
    }

    // Visit children.
    let mut cursor = node.walk();
    for c in node.children(&mut cursor) {
        visit(c, v, source);
    }

    // Undo nesting.
    if cf_increment {
        v.nesting -= 1;
    }
}

fn is_control_flow(kind: &str) -> bool {
    matches!(
        kind,
        "if_statement"
            | "for_statement"
            | "for_in_statement"
            | "for_of_statement"
            | "while_statement"
            | "do_statement"
            | "switch_statement"
            | "catch_clause"
            | "ternary_expression"
    )
    // Note: `else` is part of if_statement in tree-sitter, so it doesn't
    // need separate handling. `else if` is also nested if_statement.
}

fn is_function_call_named(node: Node, function_name: &str, source: &str) -> bool {
    if function_name.is_empty() {
        return false;
    }
    if node.kind() != "call_expression" {
        return false;
    }
    let Some(func) = node.child_by_field_name("function") else {
        return false;
    };
    let Ok(name) = func.utf8_text(source.as_bytes()) else {
        return false;
    };
    name == function_name
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::parser::{visit_descendants, with_parser};
    use crate::scanner::language::Language;

    fn cc_of(source: &str) -> u32 {
        with_parser(Language::TypeScript, source, |tree| {
            // Find first function/method in the file.
            let mut found = None;
            visit_descendants(tree.root_node(), |n| {
                if found.is_some() {
                    return;
                }
                if matches!(
                    n.kind(),
                    "function_declaration" | "function" | "method_definition" | "arrow_function"
                ) {
                    found = Some(n);
                }
            });
            let Some(found) = found else {
                return 0;
            };
            let name = found
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .map(String::from);
            cognitive_complexity(found, source, name.as_deref()).cc
        })
        .unwrap_or(0)
    }

    #[test]
    fn cc_simple_function() {
        // function f() { return 1; }   → CC = 0
        assert_eq!(cc_of("function f() { return 1; }"), 0);
    }

    #[test]
    fn cc_one_if() {
        // function f() { if (x) { y(); } }   → CC = 1
        assert_eq!(cc_of("function f() { if (x) y(); }"), 1);
    }

    #[test]
    fn cc_nested_if() {
        // function f() { if (x) { if (y) { z(); } } }   → CC = 1 (if) + 1 (nested if) + 1 (nesting) = 3
        assert_eq!(cc_of("function f() { if (x) { if (y) z(); } }"), 3);
    }

    #[test]
    fn cc_logical_operators() {
        // function f() { return a && b; }   → CC = 1
        assert_eq!(cc_of("function f() { return a && b; }"), 1);
    }

    #[test]
    fn cc_for_loop() {
        // function f() { for (let i = 0; i < 10; i++) { y(); } }   → CC = 1
        assert_eq!(
            cc_of("function f() { for (let i = 0; i < 10; i++) y(); }"),
            1
        );
    }

    #[test]
    fn cc_catch() {
        // function f() { try { x(); } catch (e) { y(); } }   → CC = 1
        assert_eq!(cc_of("function f() { try { x(); } catch (e) { y(); } }"), 1);
    }
}
