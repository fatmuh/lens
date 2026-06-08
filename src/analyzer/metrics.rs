//! TypeScript / TSX metrics via tree-sitter AST.
//!
//! Computes per-file: LOC, code/comment/blank lines, function/class/interface/
//! type/enum counts, and cyclomatic complexity (per function + total).
//! Then aggregates them project-wide.

use tree_sitter::{Node, Tree};

use crate::analyzer::parser::visit_descendants;
use crate::scanner::language::Language;

/// Information about a single function detected in a file.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
    pub complexity: u32,
    pub parameter_count: u32,
}

/// Per-file metrics.
#[derive(Debug, Clone, Default)]
pub struct FileMetrics {
    pub loc: u32,
    pub code_lines: u32,
    pub comment_lines: u32,
    pub blank_lines: u32,
    pub function_count: u32,
    pub class_count: u32,
    pub interface_count: u32,
    pub type_alias_count: u32,
    pub enum_count: u32,
    pub cyclomatic_complexity: u32,
    pub functions: Vec<FunctionInfo>,
}

/// Project-wide aggregated metrics.
#[derive(Debug, Clone)]
pub struct AggregateMetrics {
    pub total_files: u32,
    pub total_loc: u64,
    pub total_code_lines: u64,
    pub total_comment_lines: u64,
    pub total_blank_lines: u64,
    pub total_functions: u64,
    pub total_classes: u64,
    pub total_interfaces: u64,
    pub total_type_aliases: u64,
    pub total_enums: u64,
    pub total_complexity: u64,
    pub avg_complexity_per_function: f64,
    pub max_function: Option<FunctionInfo>,
    pub top_functions: Vec<FunctionInfo>,
}

/// Compute per-file metrics from a tree-sitter parse tree.
pub fn compute(tree: &Tree, source: &str, lang: Language) -> FileMetrics {
    let root = tree.root_node();
    let mut m = FileMetrics::default();

    // Line-based counts (independent of AST — useful for TSX with errors).
    count_lines(source, &mut m);

    // AST-based counts.
    let mut functions: Vec<FunctionInfo> = Vec::new();
    let mut total_complexity: u32 = 0;

    visit_descendants(root, |node| {
        match node.kind() {
            "function_declaration" | "generator_function_declaration" => {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("<anonymous>")
                    .to_string();
                let params = count_params(&node, source);
                let complexity = cyclomatic_complexity(&node, source);
                let start = node.start_position().row as u32 + 1;
                let end = node.end_position().row as u32 + 1;
                functions.push(FunctionInfo {
                    name,
                    start_line: start,
                    end_line: end,
                    complexity,
                    parameter_count: params,
                });
                m.function_count += 1;
                total_complexity += complexity;
            }
            "method_definition" => {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("<anonymous>")
                    .to_string();
                let params = count_params(&node, source);
                let complexity = cyclomatic_complexity(&node, source);
                let start = node.start_position().row as u32 + 1;
                let end = node.end_position().row as u32 + 1;
                functions.push(FunctionInfo {
                    name,
                    start_line: start,
                    end_line: end,
                    complexity,
                    parameter_count: params,
                });
                m.function_count += 1;
                total_complexity += complexity;
            }
            "arrow_function" | "function" => {
                // Try to extract a name from a parent variable declarator.
                let name =
                    arrow_function_name(&node, source).unwrap_or_else(|| "<anonymous>".to_string());
                let params = count_params(&node, source);
                let complexity = cyclomatic_complexity(&node, source);
                let start = node.start_position().row as u32 + 1;
                let end = node.end_position().row as u32 + 1;
                // We only count "named" arrow functions + simple ones, to
                // avoid double-counting nested arrows in callbacks.
                if !name.is_empty() {
                    functions.push(FunctionInfo {
                        name,
                        start_line: start,
                        end_line: end,
                        complexity,
                        parameter_count: params,
                    });
                    m.function_count += 1;
                    total_complexity += complexity;
                }
            }
            // TS + Dart shared: class_declaration
            "class_declaration" | "abstract_class_declaration" => m.class_count += 1,
            "interface_declaration" => m.interface_count += 1,
            "type_alias_declaration" => m.type_alias_count += 1,
            "enum_declaration" => m.enum_count += 1,
            // --- Dart/Flutter AST nodes ---
            // Dart uses: class_declaration, mixin_declaration, enum_declaration, extension_declaration
            // function_declaration → function_signature → name
            // method_signature → function_signature → name
            "mixin_declaration" => m.class_count += 1,
            "extension_declaration" => m.class_count += 1,
            "function_declaration" if lang == Language::Dart => {
                // Top-level Dart function
                let name = extract_dart_func_name(&node, source);
                let params = count_params_dart(&node, source);
                let start = node.start_position().row as u32 + 1;
                let end = node.end_position().row as u32 + 1;
                let complexity = cyclomatic_complexity(&node, source);
                functions.push(FunctionInfo {
                    name,
                    start_line: start,
                    end_line: end,
                    complexity,
                    parameter_count: params,
                });
                m.function_count += 1;
                total_complexity += complexity;
            }
            "method_signature" if lang == Language::Dart => {
                // Dart method — name is inside function_signature child
                let name = extract_dart_method_name(&node, source);
                let params = count_params_dart(&node, source);
                let start = node.start_position().row as u32 + 1;
                let end = node.end_position().row as u32 + 1;
                let complexity = cyclomatic_complexity(&node, source);
                functions.push(FunctionInfo {
                    name,
                    start_line: start,
                    end_line: end,
                    complexity,
                    parameter_count: params,
                });
                m.function_count += 1;
                total_complexity += complexity;
            }
            // --- End Dart ---
            _ => {}
        }
    });

    m.functions = functions;
    m.cyclomatic_complexity = total_complexity.max(1);
    m
}

fn count_lines(source: &str, m: &mut FileMetrics) {
    let mut in_block_comment = false;
    for raw_line in source.lines() {
        m.loc += 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            m.blank_lines += 1;
            continue;
        }
        // Naive: does this line contain a `//` (not inside a string) or are
        // we inside a block comment? This is approximate; for accuracy we'd
        // walk the AST. For metrics it's good enough.
        let mut is_comment_line = false;
        let bytes = trimmed.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if in_block_comment {
                if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    in_block_comment = false;
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
            if bytes[i] == b'/' && i + 1 < bytes.len() {
                if bytes[i + 1] == b'/' {
                    is_comment_line = true;
                    break;
                }
                if bytes[i + 1] == b'*' {
                    in_block_comment = true;
                    i += 2;
                    continue;
                }
            }
            i += 1;
        }
        if in_block_comment {
            // entire line was inside a block comment
            is_comment_line = true;
        }
        if is_comment_line {
            m.comment_lines += 1;
        } else {
            m.code_lines += 1;
        }
    }
}

fn count_params(node: &Node, source: &str) -> u32 {
    let Some(params) = node.child_by_field_name("parameters") else {
        return 0;
    };
    let mut n = 0u32;
    let mut cursor = params.walk();
    for child in params.children(&mut cursor) {
        if matches!(
            child.kind(),
            "required_parameter" | "optional_parameter" | "rest_pattern" | "assignment_pattern"
        ) {
            n += 1;
        }
        // Skip identifier "name" tokens for `(a, b)` style.
        if child.kind() == "identifier" {
            n += 1;
        }
    }
    let _ = source; // not used directly
    n
}

fn count_params_dart(node: &Node, source: &str) -> u32 {
    // Dart uses: function_signature → formal_parameter_list
    // method_signature → function_signature → formal_parameter_list
    let mut n = 0u32;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Look for function_signature which contains formal_parameter_list
        if child.kind() == "function_signature" {
            let mut c2 = child.walk();
            for sig_child in child.children(&mut c2) {
                if sig_child.kind() == "formal_parameter_list" {
                    n += count_dart_params_in_list(&sig_child);
                }
            }
        }
        if child.kind() == "formal_parameter_list" {
            n += count_dart_params_in_list(&child);
        }
    }
    let _ = source;
    n
}

fn count_dart_params_in_list(list: &Node) -> u32 {
    let mut n = 0u32;
    let mut cursor = list.walk();
    for child in list.children(&mut cursor) {
        if child.kind() == "formal_parameter" {
            n += 1;
        }
        // Optional positional params: (a, b)
        if child.kind() == "optional_formal_parameters" {
            let mut c2 = child.walk();
            for p in child.children(&mut c2) {
                if p.kind() == "formal_parameter" || p.kind() == "default_formal_parameter" {
                    n += 1;
                }
            }
        }
        // Named params: {a, b}
        if child.kind() == "named_formal_parameters" {
            let mut c2 = child.walk();
            for p in child.children(&mut c2) {
                if p.kind() == "named_formal_parameter"
                    || p.kind() == "default_named_parameter"
                    || p.kind() == "required_parameter"
                {
                    n += 1;
                }
            }
        }
    }
    n
}

fn extract_dart_func_name(node: &Node, source: &str) -> String {
    // function_declaration → function_signature → name: (identifier)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_signature" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    return name.to_string();
                }
            }
        }
    }
    "<anonymous>".to_string()
}

fn extract_dart_method_name(node: &Node, source: &str) -> String {
    // method_signature → function_signature → name: (identifier)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_signature" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    return name.to_string();
                }
            }
        }
        // getter/setter
        if child.kind() == "getter_signature" || child.kind() == "setter_signature" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    return name.to_string();
                }
            }
        }
    }
    "<anonymous>".to_string()
}

fn arrow_function_name(node: &Node, source: &str) -> Option<String> {
    let parent = node.parent()?;
    if parent.kind() == "variable_declarator" {
        return parent
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string());
    }
    if parent.kind() == "assignment_expression" {
        return parent
            .child_by_field_name("left")
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string());
    }
    None
}

/// Cyclomatic complexity of a node. Starts at 1, adds 1 for each branching
/// construct found in the node's subtree.
pub fn cyclomatic_complexity(node: &Node, source: &str) -> u32 {
    let mut count = 1u32;
    visit_descendants(*node, |n| match n.kind() {
        "if_statement" | "case_statement" | "catch_clause" | "ternary_expression" => count += 1,
        "for_statement" | "for_in_statement" | "for_of_statement" | "while_statement"
        | "do_statement" => count += 1,
        "binary_expression" => {
            if let Some(op) = n.child_by_field_name("operator") {
                if let Ok(t) = op.utf8_text(source.as_bytes()) {
                    if matches!(t, "&&" | "||" | "??" | "??=") {
                        count += 1;
                    }
                }
            }
        }
        _ => {}
    });
    count
}

/// Aggregate per-file metrics into a project-wide summary.
pub fn aggregate(files: &[&FileMetrics]) -> AggregateMetrics {
    let mut agg = AggregateMetrics {
        total_files: files.len() as u32,
        total_loc: 0,
        total_code_lines: 0,
        total_comment_lines: 0,
        total_blank_lines: 0,
        total_functions: 0,
        total_classes: 0,
        total_interfaces: 0,
        total_type_aliases: 0,
        total_enums: 0,
        total_complexity: 0,
        avg_complexity_per_function: 0.0,
        max_function: None,
        top_functions: Vec::new(),
    };

    let mut all_functions: Vec<FunctionInfo> = Vec::new();

    for fm in files {
        agg.total_loc += fm.loc as u64;
        agg.total_code_lines += fm.code_lines as u64;
        agg.total_comment_lines += fm.comment_lines as u64;
        agg.total_blank_lines += fm.blank_lines as u64;
        agg.total_functions += fm.function_count as u64;
        agg.total_classes += fm.class_count as u64;
        agg.total_interfaces += fm.interface_count as u64;
        agg.total_type_aliases += fm.type_alias_count as u64;
        agg.total_enums += fm.enum_count as u64;
        agg.total_complexity += fm.cyclomatic_complexity as u64;
        all_functions.extend(fm.functions.clone());
    }

    if agg.total_functions > 0 {
        agg.avg_complexity_per_function = agg.total_complexity as f64 / agg.total_functions as f64;
    }

    // Find the most complex function overall.
    agg.max_function = all_functions.iter().max_by_key(|f| f.complexity).cloned();

    // Top 10 functions by complexity.
    all_functions.sort_by(|a, b| b.complexity.cmp(&a.complexity));
    agg.top_functions = all_functions.into_iter().take(10).collect();

    agg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::parser::with_parser;
    use crate::scanner::language::Language;

    #[test]
    fn computes_simple_function_metrics() {
        let src = r#"
function add(a: number, b: number): number {
    if (a > 0) {
        return a + b;
    }
    return b;
}

class Foo {
    method(x: string): void {
        for (let i = 0; i < 10; i++) {
            console.log(x);
        }
    }
}
"#;
        let m = with_parser(Language::TypeScript, src, |t| {
            compute(t, src, Language::TypeScript)
        })
        .unwrap();

        assert!(
            m.function_count >= 2,
            "expected >= 2 functions, got {}",
            m.function_count
        );
        assert_eq!(m.class_count, 1);
        assert!(
            m.cyclomatic_complexity >= 3,
            "expected >= 3, got {}",
            m.cyclomatic_complexity
        );

        // `add` has an if (+1), `method` has a for (+1), so cyclomatic >= 3
    }

    #[test]
    fn handles_arrow_function_in_variable() {
        let src = "const double = (x: number) => x * 2;\n";
        let m = with_parser(Language::TypeScript, src, |t| {
            compute(t, src, Language::TypeScript)
        })
        .unwrap();
        assert_eq!(m.function_count, 1);
        let f = &m.functions[0];
        assert_eq!(f.name, "double");
        assert_eq!(f.parameter_count, 1);
    }
}
