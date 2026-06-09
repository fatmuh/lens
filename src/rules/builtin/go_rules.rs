//! Go-specific rules.
//!
//! Rules that detect Go anti-patterns and Go-specific issues:
//! - avoid_println: Use logging instead of fmt.Println()
//! - avoid_empty_block: Empty blocks should be removed or commented
//! - error_check: Errors should be checked after function calls
//! - avoid_init: Avoid init() functions
//! - avoid_global_var: Avoid global variables
//! - export_comment: Exported names should have comments

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

/// Avoid fmt.Print* in production Go code.
pub struct GoAvoidPrint;

impl Rule for GoAvoidPrint {
    fn id(&self) -> &'static str {
        "go/avoid-print"
    }
    fn name(&self) -> &'static str {
        "Avoid print statements"
    }
    fn description(&self) -> &'static str {
        "Use structured logging instead of fmt.Print/Printf/Println"
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::Go]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("fmt.Print")
                || trimmed.contains("fmt.Println")
                || trimmed.contains("fmt.Printf")
                || trimmed.contains("log.Println")
            {
                // Allow in test files
                if file.path.to_string_lossy().ends_with("_test.go") {
                    continue;
                }
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Use structured logging instead of fmt.Print*".to_string(),
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

/// Empty blocks should be removed or commented.
pub struct GoAvoidEmptyBlock;

impl Rule for GoAvoidEmptyBlock {
    fn id(&self) -> &'static str {
        "go/avoid-empty-block"
    }
    fn name(&self) -> &'static str {
        "Avoid empty blocks"
    }
    fn description(&self) -> &'static str {
        "Empty blocks should be removed or documented with a comment"
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[Language::Go]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            // Detect empty if/for/select/switch blocks: `if x { }`
            // Simple heuristic: line ends with `{ }` or next non-empty line is `}`
            if trimmed.ends_with("{}") || trimmed == "{}" {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Empty block. Add a comment or remove it.".to_string(),
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

/// Errors returned from functions should be checked.
pub struct GoErrorCheck;

impl Rule for GoErrorCheck {
    fn id(&self) -> &'static str {
        "go/error-check"
    }
    fn name(&self) -> &'static str {
        "Check errors"
    }
    fn description(&self) -> &'static str {
        "Returned errors should be checked, not discarded"
    }
    fn default_severity(&self) -> Severity {
        Severity::Critical
    }
    fn languages(&self) -> &[Language] {
        &[Language::Go]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            // Pattern: `err := someFunc()` followed by NO `if err != nil`
            // Simple heuristic: line assigns to `err` but next lines don't check it
            if trimmed.contains("err :=") || trimmed.contains("err,") || trimmed.contains(", err") {
                // Check if this line also checks the error inline
                if trimmed.contains("if") && trimmed.contains("err") {
                    continue;
                }
                // Check next few lines for error checking
                let mut checked = false;
                for j in 1..=3 {
                    if i + j >= lines.len() {
                        break;
                    }
                    let next = lines[i + j].trim();
                    if next.contains("if err") || next.contains("err != nil") || next == "" {
                        if next.contains("if err") || next.contains("err != nil") {
                            checked = true;
                        }
                        if next.is_empty() {
                            continue;
                        }
                        break;
                    }
                    if next.contains("err") && (next.contains("if") || next.contains("check")) {
                        checked = true;
                        break;
                    }
                }
                if !checked {
                    issues.push(Issue {
                        rule_id: self.id().to_string(),
                        severity: self.default_severity(),
                        message: "Returned error is not checked. Add `if err != nil` check."
                            .to_string(),
                        file: file.path.clone(),
                        start_line: i as u32 + 1,
                        end_line: i as u32 + 1,
                        start_column: 0,
                        end_column: 0,
                    });
                }
            }
        }
        issues
    }
}

/// Avoid init() functions — they make code hard to test and reason about.
pub struct GoAvoidInit;

impl Rule for GoAvoidInit {
    fn id(&self) -> &'static str {
        "go/avoid-init"
    }
    fn name(&self) -> &'static str {
        "Avoid init()"
    }
    fn description(&self) -> &'static str {
        "init() functions make code hard to test. Use explicit initialization."
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::Go]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("func init()") || trimmed.starts_with("func init (") {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Avoid init() functions. Use explicit initialization instead."
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

/// Avoid global variables — prefer dependency injection.
pub struct GoAvoidGlobalVar;

impl Rule for GoAvoidGlobalVar {
    fn id(&self) -> &'static str {
        "go/avoid-global-var"
    }
    fn name(&self) -> &'static str {
        "Avoid global variables"
    }
    fn description(&self) -> &'static str {
        "Global variables make code hard to test. Use dependency injection."
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn languages(&self) -> &[Language] {
        &[Language::Go]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            // var <name> = ... or var <name> <type> at package level (not inside func)
            if trimmed.starts_with("var ") && !trimmed.contains(":= ") {
                // Check it's not inside a function by seeing indentation
                if !line.starts_with('\t') && !line.starts_with("  ") {
                    // Skip var blocks (var ( ... ))
                    if trimmed == "var (" || trimmed == "var(" {
                        continue;
                    }
                    // Skip const-like patterns
                    if trimmed.contains("const") {
                        continue;
                    }
                    issues.push(Issue {
                        rule_id: self.id().to_string(),
                        severity: self.default_severity(),
                        message: "Global variable detected. Consider dependency injection."
                            .to_string(),
                        file: file.path.clone(),
                        start_line: i as u32 + 1,
                        end_line: i as u32 + 1,
                        start_column: 0,
                        end_column: 0,
                    });
                }
            }
        }
        issues
    }
}

/// Exported names should have documentation comments.
pub struct GoExportedComment;

impl Rule for GoExportedComment {
    fn id(&self) -> &'static str {
        "go/exported-comment"
    }
    fn name(&self) -> &'static str {
        "Exported names should be commented"
    }
    fn description(&self) -> &'static str {
        "Exported (public) functions, types, and variables should have doc comments"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn languages(&self) -> &[Language] {
        &[Language::Go]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            // Check for exported func/type/var/const declarations
            if (trimmed.starts_with("func ")
                || trimmed.starts_with("type ")
                || trimmed.starts_with("var ")
                || trimmed.starts_with("const "))
                && !line.starts_with('\t')
                && !line.starts_with("  ")
            {
                // Extract the name after keyword
                let name = trimmed
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .trim_start_matches('*')
                    .split('(')
                    .next()
                    .unwrap_or("");
                // Check if exported (starts with uppercase)
                if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    // Check if previous line is a comment
                    let prev_is_comment = i > 0 && {
                        let prev = lines[i - 1].trim();
                        prev.starts_with("//") || prev.starts_with("/*")
                    };
                    if !prev_is_comment {
                        issues.push(Issue {
                            rule_id: self.id().to_string(),
                            severity: self.default_severity(),
                            message: format!(
                                "Exported '{}' should have a documentation comment",
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
