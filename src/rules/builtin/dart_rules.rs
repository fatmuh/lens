//! Dart/Flutter-specific rules.
//!
//! Rules that detect Flutter anti-patterns and Dart-specific issues:
//! - avoid_print: Use logging instead of print()
//! - avoid_empty_catch: Empty catch blocks swallow errors
//! - avoid_return_types_on_setters: Setters should not have return types
//! - prefer_const_constructors: Use const constructors
//! - avoid_unnecessary_containers: Remove unnecessary Widget wrappers
//! - sized_box_for_whitespace: Use SizedBox instead of Container with no child
//! - avoid_web_libraries_in_flutter: Don't use dart:html in Flutter apps

use crate::analyzer::parser::{get_language, visit_descendants, with_parser};
use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

/// Avoid print() in production Dart code.
pub struct DartAvoidPrint;

impl Rule for DartAvoidPrint {
    fn id(&self) -> &'static str {
        "dart/avoid-print"
    }
    fn name(&self) -> &'static str {
        "Avoid print()"
    }
    fn description(&self) -> &'static str {
        "Use logging instead of print() in production code"
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::Dart]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if trimmed.contains("print(")
                && !trimmed.contains("blueprint")
                && !trimmed.contains("sprintf")
            {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Avoid print() in production code. Use the logging package instead."
                        .into(),
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

/// Avoid empty catch blocks.
pub struct DartAvoidEmptyCatch;

impl Rule for DartAvoidEmptyCatch {
    fn id(&self) -> &'static str {
        "dart/avoid-empty-catch"
    }
    fn name(&self) -> &'static str {
        "Avoid empty catch"
    }
    fn description(&self) -> &'static str {
        "Empty catch blocks silently swallow errors"
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[Language::Dart]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(lang) = get_language(Language::Dart) else {
            return issues;
        };
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).ok();
        let Some(tree) = parser.parse(source, None) else {
            return issues;
        };

        visit_descendants(tree.root_node(), |node| {
            if node.kind() == "catch_clause" {
                let body = node.child_by_field_name("body");
                if let Some(body) = body {
                    let text = body.utf8_text(source.as_bytes()).unwrap_or("{}");
                    let cleaned: String = text
                        .chars()
                        .filter(|c| !c.is_whitespace() && *c != '{' && *c != '}')
                        .collect();
                    if cleaned.is_empty() {
                        let line = node.start_position().row as u32 + 1;
                        issues.push(Issue {
                            rule_id: "dart/avoid-empty-catch".to_string(),
                            severity: self.default_severity(),
                            message: "Empty catch block. Log or handle the exception.".into(),
                            file: file.path.clone(),
                            start_line: line,
                            end_line: line,
                            start_column: 0,
                            end_column: 0,
                        });
                    }
                }
            }
        });
        issues
    }
}

/// Avoid unnecessary Container widgets.
pub struct DartAvoidUnnecessaryContainers;

impl Rule for DartAvoidUnnecessaryContainers {
    fn id(&self) -> &'static str {
        "dart/avoid-unnecessary-containers"
    }
    fn name(&self) -> &'static str {
        "Avoid unnecessary Container"
    }
    fn description(&self) -> &'static str {
        "Use SizedBox, Padding, or Align instead of Container when possible"
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::Dart]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        // Simple heuristic: Container( with only width/height or only padding
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("Container(")
                && (trimmed.contains("width:") || trimmed.contains("height:"))
            {
                // Check if it's just Container(width: x, height: y) → SizedBox
                let next_lines: String =
                    source.lines().skip(i).take(5).collect::<Vec<_>>().join(" ");
                if next_lines.contains("Container(")
                    && !next_lines.contains("child:")
                    && !next_lines.contains("decoration:")
                    && !next_lines.contains("color:")
                    && (next_lines.contains("width:") || next_lines.contains("height:"))
                {
                    issues.push(Issue {
                        rule_id: self.id().to_string(),
                        severity: self.default_severity(),
                        message:
                            "Use SizedBox instead of Container when only width/height are needed."
                                .into(),
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

/// Use const constructors where possible.
pub struct DartPreferConstConstructors;

impl Rule for DartPreferConstConstructors {
    fn id(&self) -> &'static str {
        "dart/prefer-const-constructors"
    }
    fn name(&self) -> &'static str {
        "Prefer const constructors"
    }
    fn description(&self) -> &'static str {
        "Use const with constructors when all arguments are constant"
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::Dart]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        // Detect patterns like: EdgeInsets.fromLTRB(0, 0, 0, 0) → EdgeInsets.fromLTRB(...)
        // or TextStyle(...) without const
        let const_candidates = [
            "EdgeInsets.",
            "EdgeInsets.all(",
            "EdgeInsets.only(",
            "EdgeInsets.symmetric(",
            "EdgeInsets.fromLTRB(",
            "TextStyle(",
            "Color(",
            "Size(",
        ];
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            // Check if there's a const-eligible constructor that isn't preceded by 'const'
            for candidate in &const_candidates {
                if let Some(idx) = trimmed.find(candidate) {
                    let before = &trimmed[..idx];
                    // Not const already, not in a return/type position
                    if !before.ends_with("const ") && !before.ends_with("const\t") {
                        // Check if this looks like a constructor call (has parentheses)
                        // Simple heuristic: line contains the pattern and isn't commented
                        issues.push(Issue {
                            rule_id: self.id().to_string(),
                            severity: self.default_severity(),
                            message: format!(
                                "Consider using 'const {}' for better performance.",
                                candidate
                            ),
                            file: file.path.clone(),
                            start_line: i as u32 + 1,
                            end_line: i as u32 + 1,
                            start_column: 0,
                            end_column: 0,
                        });
                        break; // one issue per line
                    }
                }
            }
        }
        issues
    }
}

/// Avoid using dart:html in Flutter apps.
pub struct DartAvoidWebLibraries;

impl Rule for DartAvoidWebLibraries {
    fn id(&self) -> &'static str {
        "dart/avoid-web-libraries-in-flutter"
    }
    fn name(&self) -> &'static str {
        "Avoid web libraries in Flutter"
    }
    fn description(&self) -> &'static str {
        "Avoid dart:html, dart:js, dart:web_gl in Flutter apps for portability"
    }
    fn default_severity(&self) -> Severity {
        Severity::Major
    }
    fn languages(&self) -> &[Language] {
        &[Language::Dart]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let web_imports = [
            "dart:html",
            "dart:js",
            "dart:js_util",
            "dart:web_gl",
            "dart:svg",
        ];
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            for imp in &web_imports {
                if trimmed.contains(imp)
                    && (trimmed.starts_with("import ") || trimmed.starts_with("export "))
                {
                    issues.push(Issue {
                        rule_id: self.id().to_string(),
                        severity: self.default_severity(),
                        message: format!(
                            "Avoid '{}' in Flutter apps. It breaks mobile/desktop compatibility.",
                            imp
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
        issues
    }
}

/// Prefer async/await over raw Futures.
pub struct DartPreferAsyncAwait;

impl Rule for DartPreferAsyncAwait {
    fn id(&self) -> &'static str {
        "dart/prefer-async-await"
    }
    fn name(&self) -> &'static str {
        "Prefer async/await"
    }
    fn description(&self) -> &'static str {
        "Use async/await instead of .then() for better readability"
    }
    fn default_severity(&self) -> Severity {
        Severity::Minor
    }
    fn languages(&self) -> &[Language] {
        &[Language::Dart]
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            // Detect .then( but not inside a test assertion
            if trimmed.contains(".then(") && !trimmed.starts_with("expect") {
                issues.push(Issue {
                    rule_id: self.id().to_string(),
                    severity: self.default_severity(),
                    message: "Prefer async/await over .then() for better readability.".into(),
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
