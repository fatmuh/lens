//! Dependency vulnerability scanner using OSV (Open Source Vulnerabilities) API.
//!
//! Parses lock files and queries https://osv.dev for known vulnerabilities.
//! Supports: npm (package-lock.json), Cargo (Cargo.lock), Go (go.sum), Pub (pubspec.lock).

pub mod lockfile;
pub mod osv;

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Serialize;

use crate::cli::DepArgs;

/// Run `lens dep` — scan dependencies for known vulnerabilities.
pub fn run_dep(args: DepArgs) -> Result<std::process::ExitCode> {
    let root = std::path::Path::new(&args.path);
    if !root.exists() {
        anyhow::bail!("path does not exist: {}", root.display());
    }

    // Discover lock files
    let packages = lockfile::discover_and_parse(root).context("discovering dependency files")?;

    if packages.is_empty() {
        println!(
            "  {} No lock files found in {}",
            "\u{2139}\u{FE0F}".dimmed(),
            root.display()
        );
        println!(
            "  {} Supported: package-lock.json, Cargo.lock, go.sum, pubspec.lock",
            "\u{2139}\u{FE0F}".dimmed()
        );
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let is_json = matches!(args.format, crate::cli::Format::Json);

    if !is_json {
        println!(
            "  {} Found {} dependencies across {} lock file(s)",
            "\u{1F50D}",
            packages.len().to_string().bold(),
            count_unique_files(&packages)
        );
    }

    if args.audit_only {
        // Just list dependencies, don't query OSV
        print_dependency_list(&packages);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    // Query OSV
    if !is_json {
        println!("  {} Querying OSV database...", "\u{1F310}");
    }

    let vulns = osv::query_batch(&packages)?;

    // Count severities
    let mut blocker = 0u32;
    let mut critical = 0u32;
    let mut major = 0u32;
    let mut minor = 0u32;
    let mut info = 0u32;
    for v in &vulns {
        match v.severity {
            crate::rules::Severity::Blocker => blocker += 1,
            crate::rules::Severity::Critical => critical += 1,
            crate::rules::Severity::Major => major += 1,
            crate::rules::Severity::Minor => minor += 1,
            crate::rules::Severity::Info => info += 1,
        }
    }

    // JSON output
    if is_json {
        print_json(&packages, &vulns, blocker, critical, major, minor, info);
        if args.gate && (blocker > 0 || critical > 0) {
            return Ok(std::process::ExitCode::from(1));
        }
        return Ok(std::process::ExitCode::SUCCESS);
    }

    // Terminal output
    if vulns.is_empty() {
        println!("  {} No known vulnerabilities found!", "\u{2705}".green());
        return Ok(std::process::ExitCode::SUCCESS);
    }

    println!();
    println!(
        "  {} Vulnerable dependencies",
        "\u{26A0}\u{FE0F}".yellow().bold()
    );
    println!("  {}", "\u{2500}".repeat(60).dimmed());

    for v in &vulns {
        let sev_emoji = match v.severity {
            crate::rules::Severity::Blocker => "\u{1F534}",
            crate::rules::Severity::Critical => "\u{1F7E0}",
            crate::rules::Severity::Major => "\u{1F7E1}",
            crate::rules::Severity::Minor => "\u{1F535}",
            crate::rules::Severity::Info => "\u{26AA}",
        };

        println!(
            "  {} {} {} {} ({} in {}@{})",
            sev_emoji,
            v.osv_id.bold(),
            v.summary.dimmed(),
            format!("[{}]", v.ecosystem).dimmed(),
            "affects".dimmed(),
            v.package.bold(),
            v.version.dimmed(),
        );
        if let Some(ref url) = v.url {
            println!("    {} {}", "\u{1F517}".dimmed(), url.dimmed());
        }
        if let Some(ref aliases) = v.aliases {
            if !aliases.is_empty() {
                println!(
                    "    {} {}",
                    "Aliases:".dimmed(),
                    aliases.join(", ").dimmed()
                );
            }
        }
    }

    println!();
    println!("  {} Summary", "\u{1F4CA}".bold());
    println!("  {}", "\u{2500}".repeat(60).dimmed());
    println!(
        "  {} dependencies scanned, {} vulnerable",
        packages.len(),
        vulns.len()
    );
    println!(
        "  {} {} critical, {} high, {} medium, {} low, {} info",
        "\u{26A0}\u{FE0F}",
        blocker.to_string().red().bold(),
        critical.to_string().red(),
        major.to_string().yellow(),
        minor.to_string().blue(),
        info
    );

    // Exit code: non-zero if any critical/high vulnerabilities
    if args.gate && (blocker > 0 || critical > 0) {
        return Ok(std::process::ExitCode::from(1));
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn count_unique_files(packages: &[lockfile::Package]) -> usize {
    let mut files = std::collections::HashSet::new();
    for p in packages {
        files.insert(&p.source_file);
    }
    files.len()
}

fn print_dependency_list(packages: &[lockfile::Package]) {
    // Group by ecosystem
    let mut groups: std::collections::BTreeMap<String, Vec<&lockfile::Package>> =
        std::collections::BTreeMap::new();
    for p in packages {
        groups.entry(p.ecosystem.clone()).or_default().push(p);
    }

    for (eco, pkgs) in &groups {
        println!("  {} ({} packages)", eco.bold(), pkgs.len());
        for p in pkgs {
            println!("    {}@{}", p.name.cyan(), p.version.dimmed());
        }
    }
}

fn print_json(
    packages: &[lockfile::Package],
    vulns: &[osv::Vulnerability],
    blocker: u32,
    critical: u32,
    major: u32,
    minor: u32,
    info: u32,
) {
    #[derive(Serialize)]
    struct VulnJson {
        osv_id: String,
        package: String,
        version: String,
        ecosystem: String,
        summary: String,
        severity: String,
        url: Option<String>,
        aliases: Option<Vec<String>>,
    }

    #[derive(Serialize)]
    struct Output {
        total_scanned: usize,
        vulnerable: usize,
        blocker: u32,
        critical: u32,
        major: u32,
        minor: u32,
        info: u32,
        vulnerabilities: Vec<VulnJson>,
    }

    let vulnerabilities: Vec<VulnJson> = vulns
        .iter()
        .map(|v| VulnJson {
            osv_id: v.osv_id.clone(),
            package: v.package.clone(),
            version: v.version.clone(),
            ecosystem: v.ecosystem.clone(),
            summary: v.summary.clone(),
            severity: format!("{:?}", v.severity).to_lowercase(),
            url: v.url.clone(),
            aliases: v.aliases.clone(),
        })
        .collect();

    let output = Output {
        total_scanned: packages.len(),
        vulnerable: vulns.len(),
        blocker,
        critical,
        major,
        minor,
        info,
        vulnerabilities,
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
