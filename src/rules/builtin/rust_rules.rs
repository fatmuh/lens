//! Rust-specific rules.
//!
//! Rules that detect Rust anti-patterns:
//! - avoid_println: Use tracing/logging instead of println!
//! - avoid_unwrap: Avoid .unwrap() in production code
//! - avoid_expect: Avoid .expect() in production code
//! - avoid_todo: Avoid todo!() macro
//! - avoid_unimplemented: Avoid unimplemented!() macro
//! - explicit_types: Public functions should have explicit return types

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

/// Avoid println! in production Rust code.
pub struct RustAvoidPrintln;

impl Rule for RustAvoidPrintln {
    fn id(&self) -> &'static str {
        "rust/avoid-println"
    }
    fn name(&self) -> &'static str {
        "Avoid println!"
    }
    fn description(&self) -> &'static str {
        "Use tracing or the `log` crate instead of println! in production"
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let is_test = file.path.to_string_lossy().contains("/tests/")
            || file.path.to_string_lossy().ends_with("_test.rs")
            || file.path.to_string_lossy().ends_with("test_.rs")
            || file.path.to_string_lossy().contains("\\tests\\");
        if is_test {
            return issues;
        }
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("println!(")
                || trimmed.contains("eprintln!(")
                || trimmed.contains("dbg!(")
            {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Use `tracing::info!` or `log::info!` instead of println!".to_string(),
                    file: file.path.clone(),
                    start_line: i as u32 + 1,
                    end_line: i as u32 + 1,
                    start_column: 0,
                    end_column: 0,
                });
            }
        }
        issues
    }
}

/// Avoid .unwrap() in production — use proper error handling.
pub struct RustAvoidUnwrap;

impl Rule for RustAvoidUnwrap {
    fn id(&self) -> &'static str {
        "rust/avoid-unwrap"
    }
    fn name(&self) -> &'static str {
        "Avoid unwrap()"
    }
    fn description(&self) -> &'static str {
        "Use `?`, `ok_or()`, or match instead of .unwrap() in production code"
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let is_test = file.path.to_string_lossy().contains("/tests/")
            || file.path.to_string_lossy().ends_with("_test.rs")
            || file.path.to_string_lossy().ends_with("test_.rs")
            || file.path.to_string_lossy().contains("\\tests\\");
        if is_test {
            return issues;
        }
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            // Check for .unwrap() but allow in tests and examples
            if trimmed.contains(".unwrap()") {
                // Allow in obvious test contexts
                if trimmed.contains("#[test]")
                    || trimmed.contains("fn test_")
                    || trimmed.contains("#[cfg(test)]")
                {
                    continue;
                }
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Avoid .unwrap() — use `?`, `ok_or()`, or match for error handling"
                        .to_string(),
                    file: file.path.clone(),
                    start_line: i as u32 + 1,
                    end_line: i as u32 + 1,
                    start_column: 0,
                    end_column: 0,
                });
            }
        }
        issues
    }
}

/// Avoid .expect() in production.
pub struct RustAvoidExpect;

impl Rule for RustAvoidExpect {
    fn id(&self) -> &'static str {
        "rust/avoid-expect"
    }
    fn name(&self) -> &'static str {
        "Avoid expect()"
    }
    fn description(&self) -> &'static str {
        "Use `?` or proper error propagation instead of .expect()"
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let is_test = file.path.to_string_lossy().contains("/tests/")
            || file.path.to_string_lossy().ends_with("_test.rs")
            || file.path.to_string_lossy().ends_with("test_.rs")
            || file.path.to_string_lossy().contains("\\tests\\");
        if is_test {
            return issues;
        }
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains(".expect(") {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Use `?` or `.map_err()` for error propagation instead of .expect()"
                        .to_string(),
                    file: file.path.clone(),
                    start_line: i as u32 + 1,
                    end_line: i as u32 + 1,
                    start_column: 0,
                    end_column: 0,
                });
            }
        }
        issues
    }
}

/// Avoid todo!() macro — signals incomplete code.
pub struct RustAvoidTodo;

impl Rule for RustAvoidTodo {
    fn id(&self) -> &'static str {
        "rust/avoid-todo"
    }
    fn name(&self) -> &'static str {
        "Avoid todo!()"
    }
    fn description(&self) -> &'static str {
        "Complete the implementation instead of using todo!()"
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("todo!()") || trimmed.contains("todo!(") {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Complete this implementation — todo!() will panic at runtime"
                        .to_string(),
                    file: file.path.clone(),
                    start_line: i as u32 + 1,
                    end_line: i as u32 + 1,
                    start_column: 0,
                    end_column: 0,
                });
            }
        }
        issues
    }
}

/// Avoid unimplemented!() macro.
pub struct RustAvoidUnimplemented;

impl Rule for RustAvoidUnimplemented {
    fn id(&self) -> &'static str {
        "rust/avoid-unimplemented"
    }
    fn name(&self) -> &'static str {
        "Avoid unimplemented!()"
    }
    fn description(&self) -> &'static str {
        "Implement the function instead of using unimplemented!()"
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("unimplemented!()") || trimmed.contains("unimplemented!(") {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "unimplemented!() will panic at runtime. Implement this function."
                        .to_string(),
                    file: file.path.clone(),
                    start_line: i as u32 + 1,
                    end_line: i as u32 + 1,
                    start_column: 0,
                    end_column: 0,
                });
            }
        }
        issues
    }
}

/// Public functions should have explicit return types.
pub struct RustExplicitTypes;

impl Rule for RustExplicitTypes {
    fn id(&self) -> &'static str {
        "rust/explicit-return-type"
    }
    fn name(&self) -> &'static str {
        "Explicit return types"
    }
    fn description(&self) -> &'static str {
        "Public functions should have explicit return types for documentation"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            // Look for: pub fn name(...) {  (without -> Type)
            if trimmed.starts_with("pub fn ") || trimmed.starts_with("pub async fn ") {
                // Check if the line (or next few) contains -> for return type
                let block = if i + 3 < lines.len() {
                    lines[i..=i + 3].join(" ")
                } else {
                    lines[i..].join(" ")
                };
                let has_return_arrow = block.contains("->");
                // Check if function body starts with { without ->
                if !has_return_arrow && block.contains("{") {
                    let name = trimmed
                        .split_whitespace()
                        .nth(2)
                        .unwrap_or("")
                        .split('(')
                        .next()
                        .unwrap_or("");
                    if !name.is_empty() {
                        issues.push(Issue {
                            rule_id: self.id().to_string(),
                            severity: self.default_severity(),
                            message: format!(
                                "Public function '{}' should have an explicit return type",
                                name
                            ),
                            file: file.path.clone(),
                            start_line: i as u32 + 1,
                            end_line: i as u32 + 1,
                            start_column: 0,
                            end_column: 0,
                        });
                    }
                }
            }
        }
        issues
    }
}
