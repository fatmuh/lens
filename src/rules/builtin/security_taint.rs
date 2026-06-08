//! Security taint analysis rule.
//!
//! Detects vulnerabilities by tracking data flow from user input (sources)
//! to dangerous operations (sinks). Covers:
//! - SQL Injection
//! - XSS (Cross-Site Scripting)
//! - SSRF (Server-Side Request Forgery)
//! - Command Injection
//! - Path Traversal
//! - Prototype Pollution
//! - Open Redirect
//! - Log Injection

use crate::analyzer::taint;
use crate::analyzer::FileAnalysis;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

/// Security taint analysis rule — detects 8 vulnerability classes.
pub struct SecurityTaint;

impl Rule for SecurityTaint {
    fn id(&self) -> &'static str {
        "security/taint-analysis"
    }
    fn name(&self) -> &'static str {
        "Security taint analysis"
    }
    fn description(&self) -> &'static str {
        "Detects SQL Injection, XSS, SSRF, Command Injection, Path Traversal, \
         Prototype Pollution, Open Redirect, and Log Injection by tracking \
         user input to dangerous sinks."
    }
    fn default_severity(&self) -> Severity {
        Severity::Blocker
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
        let Some(lang) = file.language else {
            return vec![];
        };
        let vulns = taint::analyze(source, lang);
        vulns
            .into_iter()
            .map(|v| {
                let severity = match v.vuln_type.as_str() {
                    "SQL Injection" | "Command Injection" => Severity::Blocker,
                    "XSS" | "SSRF" | "Path Traversal" => Severity::Critical,
                    "Prototype Pollution" | "Open Redirect" => Severity::Major,
                    "Log Injection" => Severity::Minor,
                    _ => Severity::Major,
                };
                Issue {
                    rule_id: v.rule_id,
                    severity,
                    message: format!("[{}] {}", v.vuln_type, v.message),
                    file: file.path.clone(),
                    start_line: v.line,
                    end_line: v.line,
                    start_column: 0,
                    end_column: 0,
                }
            })
            .collect()
    }
}
