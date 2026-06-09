//! Python-specific rules.
//!
//! Rules that detect Python anti-patterns and Python-specific issues:
//! - avoid_print: Use logging instead of print()
//! - avoid_bare_except: Avoid bare except: clauses
//! - avoid_mutable_default: Avoid mutable default arguments
//! - avoid_global_var: Avoid global variables
//! - avoid_todo: Avoid TODO/FIXME/HACK comments
//! - avoid_star_import: Avoid wildcard imports (from x import *)
//! - explicit_return_type: Functions should have return type annotations
//! - docstring_required: Public functions should have docstrings

use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

/// Avoid print() in production Python code.
pub struct PythonAvoidPrint;

impl Rule for PythonAvoidPrint {
    fn id(&self) -> &'static str {
        "python/avoid-print"
    }
    fn name(&self) -> &'static str {
        "Avoid print statements"
    }
    fn description(&self) -> &'static str {
        "Use logging module instead of print() for production code"
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::Python]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let path_str = file.path.to_string_lossy();
        // Skip test files
        if path_str.ends_with("_test.py")
            || path_str.ends_with("test_.py")
            || path_str.contains("/tests/")
            || path_str.contains("\\tests\\")
            || path_str.contains("/test/")
            || path_str.contains("\\test\\")
        {
            return issues;
        }
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if trimmed.contains("print(") {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Use logging instead of print()".to_string(),
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

/// Avoid bare except: clauses — catch specific exceptions instead.
pub struct PythonAvoidBareExcept;

impl Rule for PythonAvoidBareExcept {
    fn id(&self) -> &'static str {
        "python/avoid-bare-except"
    }
    fn name(&self) -> &'static str {
        "Avoid bare except"
    }
    fn description(&self) -> &'static str {
        "Bare except: catches all exceptions including KeyboardInterrupt. Use specific exception types."
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[Language::Python]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            // Match "except:" (bare except) but not "except Exception:" or "except ValueError:"
            if trimmed == "except:"
                || trimmed.starts_with("except:")
                    && trimmed.trim_end_matches(':').trim() == "except"
            {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Bare except catches all exceptions. Catch specific exception types instead.".to_string(),
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

/// Avoid mutable default arguments (lists, dicts, sets).
pub struct PythonAvoidMutableDefault;

impl Rule for PythonAvoidMutableDefault {
    fn id(&self) -> &'static str {
        "python/avoid-mutable-default"
    }
    fn name(&self) -> &'static str {
        "Avoid mutable default arguments"
    }
    fn description(&self) -> &'static str {
        "Mutable default arguments (list, dict, set) are shared across calls. Use None and initialize inside."
    }
    fn default_severity(&self) -> Severity {
        Severity::Critical
    }
    fn languages(&self) -> &[Language] {
        &[Language::Python]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            // Match def foo(x=[]) or def foo(x={}) or def foo(x=set())
            if trimmed.starts_with("def ") {
                // Check for mutable defaults: =[], ={}, =set(), =list(), =dict()
                if trimmed.contains("=[]")
                    || trimmed.contains("={}")
                    || trimmed.contains("=set()")
                    || trimmed.contains("=list()")
                    || trimmed.contains("=dict()")
                {
                    issues.push(Issue {
                        rule_id: self.id().to_string(),
                        severity: self.default_severity(),
                        message:
                            "Mutable default argument. Use None and initialize inside the function."
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

/// Avoid global variables — use dependency injection or module-level constants.
pub struct PythonAvoidGlobalVar;

impl Rule for PythonAvoidGlobalVar {
    fn id(&self) -> &'static str {
        "python/avoid-global-var"
    }
    fn name(&self) -> &'static str {
        "Avoid global variables"
    }
    fn description(&self) -> &'static str {
        "Global variables make code hard to test. Use dependency injection or constants."
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn languages(&self) -> &[Language] {
        &[Language::Python]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut in_function = false;
        let mut indent_level = 0;
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            // Track indentation to detect module-level vs function-level
            let line_indent = line.len() - line.trim_start().len();
            if line_indent == 0 {
                in_function = false;
            }
            if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                in_function = true;
                indent_level = line_indent;
                continue;
            }
            // Only report at module level (indent == 0)
            if !in_function && line_indent == 0 {
                // Skip constants (UPPER_CASE), imports, __all__, __version__, etc.
                let is_constant = trimmed.chars().next().map_or(false, |c| c.is_uppercase())
                    && trimmed.contains('=')
                    && trimmed
                        .chars()
                        .take_while(|c| *c != '=')
                        .all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit());
                let is_dunder = trimmed.starts_with("__") && trimmed.ends_with("__");
                if is_constant
                    || is_dunder
                    || trimmed.starts_with("import ")
                    || trimmed.starts_with("from ")
                {
                    continue;
                }
                // Detect variable assignments (x = ...) but not function/class definitions
                if trimmed.contains('=')
                    && !trimmed.starts_with("def ")
                    && !trimmed.starts_with("class ")
                    && !trimmed.starts_with("@")
                    && !trimmed.contains("==")
                    && !trimmed.contains("!=")
                    && !trimmed.contains("<=")
                    && !trimmed.contains(">=")
                {
                    // Only report if it looks like a simple assignment (not a comparison)
                    if trimmed.contains("= ") || trimmed.ends_with("=") {
                        // Skip type annotations without assignment (x: int)
                        if trimmed.contains(": ") && !trimmed.contains("= ") {
                            continue;
                        }
                        issues.push(Issue {
                            rule_id: self.id().to_string(),
                            severity: self.default_severity(),
                            message: "Global variable detected. Consider using dependency injection or module-level constants.".to_string(),
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

/// Avoid TODO/FIXME/HACK comments — track issues in your issue tracker.
pub struct PythonAvoidTodo;

impl Rule for PythonAvoidTodo {
    fn id(&self) -> &'static str {
        "python/avoid-todo"
    }
    fn name(&self) -> &'static str {
        "Avoid TODO/FIXME comments"
    }
    fn description(&self) -> &'static str {
        "Track technical debt in your issue tracker, not in code comments"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn languages(&self) -> &[Language] {
        &[Language::Python]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim().to_uppercase();
            if trimmed.contains("TODO")
                || trimmed.contains("FIXME")
                || trimmed.contains("HACK")
                || trimmed.contains("XXX")
            {
                // Only flag actual comments
                let orig = line.trim();
                if orig.starts_with('#') || orig.starts_with("\"\"\"") || orig.starts_with("'''") {
                    issues.push(Issue {
                        rule_id: self.id().to_string(),
                        severity: self.default_severity(),
                        message: "TODO/FIXME/HACK comment found. Track in issue tracker instead."
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

/// Avoid wildcard imports (from x import *).
pub struct PythonAvoidStarImport;

impl Rule for PythonAvoidStarImport {
    fn id(&self) -> &'static str {
        "python/avoid-star-import"
    }
    fn name(&self) -> &'static str {
        "Avoid wildcard imports"
    }
    fn description(&self) -> &'static str {
        "Wildcard imports pollute the namespace and make dependencies unclear. Import specific names."
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[Language::Python]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("from ") && trimmed.contains(" import *") {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message:
                        "Wildcard import. Import specific names instead of using 'from x import *'."
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

/// Functions should have return type annotations.
pub struct PythonExplicitReturnType;

impl Rule for PythonExplicitReturnType {
    fn id(&self) -> &'static str {
        "python/explicit-return-type"
    }
    fn name(&self) -> &'static str {
        "Explicit return type"
    }
    fn description(&self) -> &'static str {
        "Public functions should have return type annotations for better documentation and type checking"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn languages(&self) -> &[Language] {
        &[Language::Python]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let path_str = file.path.to_string_lossy();
        // Skip test files and __init__.py
        if path_str.ends_with("_test.py")
            || path_str.ends_with("test_.py")
            || path_str.ends_with("__init__.py")
        {
            return issues;
        }
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            // Match def foo(...) but NOT def foo(...) -> type:
            if trimmed.starts_with("def ") && trimmed.contains('(') && trimmed.contains(':') {
                // Check if it has -> return annotation
                // def foo(x: int) -> str:
                let has_return = trimmed.contains("->");
                // Skip private methods (starting with _)
                if trimmed.contains("def _") {
                    continue;
                }
                if !has_return {
                    // Skip __init__, __str__, etc.
                    if trimmed.contains("def __") {
                        continue;
                    }
                    issues.push(Issue {
                        rule_id: self.id().to_string(),
                        severity: self.default_severity(),
                        message: "Function missing return type annotation. Add '-> ReturnType'."
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

/// Public functions should have docstrings.
pub struct PythonDocstringRequired;

impl Rule for PythonDocstringRequired {
    fn id(&self) -> &'static str {
        "python/docstring-required"
    }
    fn name(&self) -> &'static str {
        "Docstring required"
    }
    fn description(&self) -> &'static str {
        "Public functions and classes should have docstrings"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn languages(&self) -> &[Language] {
        &[Language::Python]
    }
    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let path_str = file.path.to_string_lossy();
        if path_str.ends_with("_test.py") || path_str.ends_with("test_.py") {
            return issues;
        }
        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            // Check for public def or class (not starting with _)
            if (trimmed.starts_with("def ") || trimmed.starts_with("class "))
                && !trimmed.contains("def _")
                && !trimmed.contains("class _")
            {
                // Skip __dunder__ methods
                if trimmed.contains("def __") {
                    continue;
                }
                // Check if next non-empty line is a docstring (""" or ''')
                let mut has_docstring = false;
                for j in (i + 1)..lines.len().min(i + 4) {
                    let next = lines[j].trim();
                    if next.is_empty() {
                        continue;
                    }
                    if next.starts_with("\"\"\"")
                        || next.starts_with("'''")
                        || next.starts_with("r\"\"\"")
                        || next.starts_with("r'''")
                    {
                        has_docstring = true;
                    }
                    break;
                }
                if !has_docstring {
                    let kind = if trimmed.starts_with("class ") {
                        "Class"
                    } else {
                        "Function"
                    };
                    issues.push(Issue {
                        rule_id: self.id().to_string(),
                        severity: self.default_severity(),
                        message: format!("{} missing docstring. Add a docstring.", kind),
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
