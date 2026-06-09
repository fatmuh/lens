//! User-defined custom rules loaded from `quality-gate.toml`.
//!
//! Custom rules are regex-based: Lens scans each line of source files
//! matching the configured `languages` and flags matches as issues.

use crate::analyzer::FileAnalysis;
use crate::config::CustomRuleConfig;
use crate::rules::{Issue, Rule, Severity};
use crate::scanner::language::Language;

/// A custom rule loaded from config. Static strings are leaked once on creation.
pub struct CustomRule {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    severity: Severity,
    languages: &'static [Language],
    pattern: regex::Regex,
    message: String,
}

impl CustomRule {
    /// Build a custom rule from config. Returns `None` if the regex is invalid.
    pub fn from_config(cfg: &CustomRuleConfig) -> Option<Self> {
        let pattern = regex::Regex::new(&cfg.pattern).ok()?;
        let severity = match cfg.severity.to_lowercase().as_str() {
            "blocker" => Severity::Blocker,
            "critical" => Severity::Critical,
            "major" => Severity::Major,
            "minor" => Severity::Minor,
            "info" => Severity::Info,
            _ => Severity::Major,
        };
        let languages: Vec<Language> = cfg
            .languages
            .iter()
            .filter_map(|l| l.parse::<Language>().ok())
            .collect();

        // Leak once — fine for a CLI tool.
        let id: &'static str = Box::leak(cfg.id.clone().into_boxed_str());
        let name: &'static str = Box::leak(cfg.name.clone().into_boxed_str());
        let description: &'static str = Box::leak(cfg.description.clone().into_boxed_str());
        let languages: &'static [Language] = Box::leak(languages.into_boxed_slice());

        Some(Self {
            id,
            name,
            description,
            severity,
            languages,
            pattern,
            message: cfg.message.clone(),
        })
    }

    /// Build all custom rules from config, logging warnings for invalid ones.
    pub fn all_from_config(configs: &[CustomRuleConfig]) -> Vec<Box<dyn Rule>> {
        let mut rules = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();
        for cfg in configs {
            if !seen_ids.insert(cfg.id.clone()) {
                eprintln!(
                    "  {} Duplicate custom rule id '{}', skipping.",
                    "\u{26A0}".yellow(),
                    cfg.id
                );
                continue;
            }
            match Self::from_config(cfg) {
                Some(_rule) => {
                    eprintln!(
                        "  {} Loaded custom rule '{}' ({})",
                        "\u{2713}".green(),
                        cfg.id,
                        cfg.pattern,
                    );
                    rules.push(Box::new(_rule) as Box<dyn Rule>);
                }
                None => eprintln!(
                    "  {} Invalid regex in custom rule '{}': {}",
                    "\u{26A0}".yellow(),
                    cfg.id,
                    cfg.pattern
                ),
            }
        }
        rules
    }
}

impl Rule for CustomRule {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn description(&self) -> &'static str {
        self.description
    }
    fn default_severity(&self) -> Severity {
        self.severity
    }
    fn languages(&self) -> &[Language] {
        self.languages
    }

    fn check(&self, file: &FileAnalysis, source: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let _lang = file.language;
        let _matched_lang =
            self.languages.is_empty() || _lang.map_or(false, |l| self.languages.contains(&l));
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//")
                || trimmed.starts_with("#")
                || trimmed.starts_with("/*")
                || trimmed.starts_with("*")
            {
                continue;
            }
            for cap in self.pattern.captures_iter(line) {
                let m = cap.get(0).unwrap();
                let msg = if self.message.contains("$0") {
                    self.message.replace("$0", m.as_str())
                } else if self.message.is_empty() {
                    format!("Matched pattern: {}", m.as_str())
                } else {
                    self.message.clone()
                };
                issues.push(Issue {
                    rule_id: self.id.to_string(),
                    severity: self.default_severity(),
                    message: msg,
                    file: file.path.clone(),
                    start_line: i as u32 + 1,
                    end_line: i as u32 + 1,
                    start_column: m.start() as u32,
                    end_column: m.end() as u32,
                });
            }
        }
        issues
    }
}

use owo_colors::OwoColorize;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CustomRuleConfig;

    #[test]
    fn test_custom_rule_regex_match() {
        let cfg = CustomRuleConfig {
            id: "test-rule".into(),
            name: "Test".into(),
            description: "Test rule".into(),
            severity: "blocker".into(),
            languages: vec!["typescript".into()],
            pattern: "sk-[a-zA-Z0-9]{20,}".into(),
            message: "Found secret.".into(),
        };
        let rule = CustomRule::from_config(&cfg).unwrap();
        let file = FileAnalysis {
            path: std::path::PathBuf::from("test.ts"),
            language: Some(Language::TypeScript),
            analyzed: true,
            metrics: None,
            tokens: None,
            nosonar_count: 0,
            issues: Vec::new(),
        };
        let source = r#"const apiKey = "sk-abc12345678901234567890123456"; // BAD"#;
        let issues = rule.check(&file, source);
        assert_eq!(
            issues.len(),
            1,
            "Should detect hardcoded secret, got {} issues",
            issues.len()
        );
        assert_eq!(issues[0].rule_id, "test-rule");
        assert_eq!(issues[0].start_line, 1);
    }
}
