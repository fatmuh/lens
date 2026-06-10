//! `lens rules` subcommand — list all available rules with descriptions
//! and severities. Backed by the [`crate::rules`] registry.

use std::process::ExitCode;

use owo_colors::OwoColorize;

use crate::cli::{Format, RulesArgs};
use crate::rules::{Rule, RuleRegistry, Severity};

/// Print a list of all available rules.
pub fn list(args: RulesArgs) -> anyhow::Result<ExitCode> {
    // Try to load config from current directory to pick up custom rules
    let cfg = crate::config::Config::load_for_root(None, &std::path::PathBuf::from("."))
        .unwrap_or_default();
    let registry = RuleRegistry::with_config(&cfg.rules);
    let rules = registry.rules();
    match args.format {
        Format::Json => print_json(rules),
        Format::Terminal | Format::Html | Format::Sarif => print_terminal(rules, &args),
    }
    Ok(ExitCode::SUCCESS)
}

fn print_terminal(rules: &[Box<dyn Rule>], args: &RulesArgs) {
    println!("{}", "Available rules".bold().cyan());
    println!("{}", "─".repeat(72).dimmed());
    let filtered: Vec<&Box<dyn Rule>> = if let Some(lang) = &args.language {
        rules
            .iter()
            .filter(|r| {
                r.languages()
                    .iter()
                    .any(|l| format!("{:?}", l).eq_ignore_ascii_case(lang))
            })
            .collect()
    } else {
        rules.iter().collect()
    };
    for r in &filtered {
        let sev = format_severity(r.default_severity());
        let langs = if r.languages().is_empty() {
            "all".to_string()
        } else {
            r.languages()
                .iter()
                .map(|l| format!("{:?}", l).to_lowercase())
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!(
            "  {} {} {} [{}]",
            format!("{:<24}", r.id()).cyan(),
            format!("{:<10}", sev).yellow(),
            r.name().bold(),
            langs.dimmed()
        );
        if args.verbose {
            println!("      {}", r.description().dimmed());
        }
    }
    println!();
    println!(
        "{} {} rules listed{}",
        "ℹ".dimmed(),
        filtered.len(),
        if args.verbose {
            " (with descriptions)"
        } else {
            " (use -v for descriptions)"
        }
    );
}

fn print_json(rules: &[Box<dyn Rule>]) {
    #[derive(serde::Serialize)]
    struct RuleJson {
        id: &'static str,
        name: &'static str,
        description: &'static str,
        default_severity: Severity,
        languages: Vec<String>,
    }
    let out: Vec<RuleJson> = rules
        .iter()
        .map(|r| RuleJson {
            id: r.id(),
            name: r.name(),
            description: r.description(),
            default_severity: r.default_severity(),
            languages: r
                .languages()
                .iter()
                .map(|l| format!("{:?}", l).to_lowercase())
                .collect(),
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}

fn format_severity(s: Severity) -> &'static str {
    match s {
        Severity::Blocker => "BLOCKER",
        Severity::Critical => "CRITICAL",
        Severity::Major => "MAJOR",
        Severity::Minor => "MINOR",
        Severity::Info => "INFO",
    }
}
