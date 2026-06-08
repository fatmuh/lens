//! Scanner orchestration. Ties together discovery, language detection,
//! NOSONAR parsing, and the analyzer (Phase 1: metrics + duplication).

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use anyhow::{Context, Result};
use chrono::{Local, Utc};
use owo_colors::OwoColorize;
use tracing::info;

use crate::analyzer::{
    self,
    duplication::{DuplicationMode, DuplicationReport},
    metrics::AggregateMetrics,
    AnalyzeConfig, FileAnalysis, ProjectAnalysis,
};
use crate::cli::{CiArgs, Format, ScanArgs};
use crate::config::Config;
use crate::coverage::{self, CoverageReport};
use crate::rules::{Issue, RuleRegistry, Severity};
use crate::state;
use crate::util;

pub mod discovery;
pub mod language;
pub mod nosonar;
pub mod rules;

#[derive(Debug, Clone)]
pub struct ScanContext {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
    pub config: Config,
    /// Resolved config path (`None` if no config file was found).
    #[allow(dead_code)]
    pub config_path: Option<PathBuf>,
}

impl ScanContext {
    pub fn new(
        root: PathBuf,
        files: Vec<PathBuf>,
        config: Config,
        config_path: Option<PathBuf>,
    ) -> Self {
        Self {
            root,
            files,
            config,
            config_path,
        }
    }
}

/// Entry point of `lens scan`.
pub fn run(config_arg: Option<PathBuf>, args: ScanArgs) -> Result<ExitCode> {
    if args.no_color {
        std::env::set_var("NO_COLOR", "1");
    }

    let scan_root = args
        .path
        .canonicalize()
        .unwrap_or_else(|_| args.path.clone());
    let config_path = Config::resolve_path(config_arg.as_deref(), &scan_root);
    let config = Config::load(config_path.as_deref())
        .with_context(|| format!("loading config from {:?}", config_path))?;

    let display_root = util::path::normalize(&scan_root);
    let display_config = config_path.as_ref().map(|p| util::path::normalize(p));

    if args.verbose {
        info!("scan root: {}", display_root.display());
        if let Some(p) = &display_config {
            info!("config:    {}", p.display());
        } else {
            info!("config:    <defaults>");
        }
    }

    if args.dry_run {
        println!("{}", "✓ Configuration is valid.".green());
        return Ok(ExitCode::SUCCESS);
    }

    let started = Instant::now();

    let files = discovery::scan(&scan_root, &config.scan, args.no_gitignore)
        .context("discovering files")?;

    info!("discovered {} file(s)", files.len());

    if args.verbose {
        print_language_breakdown(&files);
    }

    // Phase 1: run the analyzer (metrics + duplication) over all files in parallel.
    let duplication_mode = if args.token_mode {
        DuplicationMode::Token
    } else {
        DuplicationMode::parse(&config.duplication.mode).unwrap_or(DuplicationMode::Sonar)
    };
    let analyze_cfg = AnalyzeConfig {
        duplication_mode,
        min_duplicate_tokens: config.duplication.min_tokens,
        min_duplicate_lines: args
            .min_duplicate_lines
            .unwrap_or(config.duplication.min_lines),
        normalize_identifiers: args.normalize_identifiers
            || config.duplication.normalize_identifiers,
        k_shingle: 5,
        winnow_window: 10,
        min_file_size_for_complexity: 0,
        rules: RuleRegistry::with_config(&config.rules),
    };
    let analysis = run_analyzer(&files, &analyze_cfg, &args);
    let nosonar_total: usize = analysis.files.iter().map(|a| a.nosonar_count).sum();

    // Phase 3: parse coverage reports (if any). Missing files are silently
    // skipped. UT/IT paths come from CLI flags and override the
    // `coverage.ut_paths` / `coverage.it_paths` config keys.
    let coverage_paths: Vec<PathBuf> = config
        .coverage
        .report_paths
        .iter()
        .map(|p| scan_root.join(p))
        .collect();
    let ut_paths: Vec<PathBuf> = if !args.coverage_ut.is_empty() {
        args.coverage_ut.iter().map(|p| scan_root.join(p)).collect()
    } else {
        config
            .coverage
            .ut_paths
            .iter()
            .map(|p| scan_root.join(p))
            .collect()
    };
    let it_paths: Vec<PathBuf> = if !args.coverage_it.is_empty() {
        args.coverage_it.iter().map(|p| scan_root.join(p)).collect()
    } else {
        config
            .coverage
            .it_paths
            .iter()
            .map(|p| scan_root.join(p))
            .collect()
    };
    let (mut coverage_report, ut_report, it_report) =
        coverage::parse_with_categories(&coverage_paths, &ut_paths, &it_paths);
    apply_coverage_excludes(&mut coverage_report, &config.coverage.exclude, &scan_root);
    // Populate UT/IT fields.
    coverage_report.ut_lines = ut_report.total_lines;
    coverage_report.ut_covered_lines = ut_report.covered_lines;
    coverage_report.ut_coverage_percent = ut_report.coverage_percent;
    coverage_report.it_lines = it_report.total_lines;
    coverage_report.it_covered_lines = it_report.covered_lines;
    coverage_report.it_coverage_percent = it_report.coverage_percent;
    // Compute new-code coverage if state exists.
    let snap = state::Snapshot::load(&scan_root);
    if !snap.files.is_empty() {
        coverage_report.compute_new_coverage(&snap, None);
    }

    let duration = started.elapsed();
    let duration_ms = duration.as_millis() as u64;

    let ctx = ScanContext::new(scan_root, files, config.clone(), config_path);

    match args.format {
        Format::Terminal => report_terminal(
            &ctx,
            &display_config,
            &analysis,
            &coverage_report,
            nosonar_total,
            duration,
            args.new_code,
            args.since_days,
        ),
        Format::Json => report_json(
            &ctx,
            &display_config,
            &analysis,
            &coverage_report,
            duration_ms,
            args.output.as_ref(),
        )?,
        Format::Html => report_html(
            &ctx,
            &display_config,
            &analysis,
            &coverage_report,
            nosonar_total,
            duration,
            args.output.as_ref(),
        )?,
        Format::Sarif => report_sarif(&ctx, &analysis, nosonar_total, args.output.as_ref())?,
    }

    if args.watch {
        println!("{}", "⚠ Watch mode not yet implemented (Phase 6).".yellow());
    }

    if args.gate {
        let gate_passed = evaluate_gate(
            &analysis,
            &coverage_report,
            config.duplication.fail_above_percent,
            config.coverage.fail_below_percent,
            args.max_rating,
        );
        if !gate_passed {
            return Ok(ExitCode::from(1));
        }
    }

    // Save state snapshot for next scan's new-code tracking.
    save_state_snapshot(&ctx.root, &analysis);

    Ok(ExitCode::SUCCESS)
}

/// Save a snapshot of file hashes + issues to `.lens/state.json` for
/// next-scan new-code tracking.
fn save_state_snapshot(scan_root: &Path, analysis: &ProjectAnalysis) {
    let mut snap = state::Snapshot::default();
    snap.scan_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    for f in &analysis.files {
        // Use a relative path so it matches what coverage tools (LCOV etc.)
        // report. The `\\?\` Windows UNC prefix must be stripped first.
        let normalized = util::path::normalize(&f.path);
        let s = normalized.to_string_lossy().to_string();
        // Manual prefix strip (Path::strip_prefix is case-sensitive and
        // brittle on Windows). Use a case-insensitive suffix match.
        let scan_str = util::path::normalize(scan_root)
            .to_string_lossy()
            .to_string();
        let rel = if s.len() > scan_str.len() && s[..scan_str.len()].eq_ignore_ascii_case(&scan_str)
        {
            let mut rest = &s[scan_str.len()..];
            // Strip leading separator.
            while rest.starts_with('\\') || rest.starts_with('/') {
                rest = &rest[1..];
            }
            rest.to_string()
        } else {
            s.clone()
        };
        let rel = rel.replace('\\', "/");
        let hash = std::fs::read(&f.path)
            .ok()
            .and_then(|bytes| {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut h = DefaultHasher::new();
                bytes.hash(&mut h);
                Some(format!("{:016x}", h.finish()))
            })
            .unwrap_or_default();
        let tracked: Vec<state::TrackedIssue> = f
            .issues
            .iter()
            .map(|i| {
                let key = state::Snapshot::issue_key(i);
                state::TrackedIssue {
                    key,
                    rule_id: i.rule_id.clone(),
                    line: i.start_line,
                    message: i.message.clone(),
                }
            })
            .collect();
        snap.files.insert(
            rel,
            state::FileSnapshot {
                hash,
                issues: tracked,
            },
        );
    }
    if let Err(e) = snap.save(scan_root) {
        eprintln!("warning: failed to save state snapshot: {e}");
    }
}

fn path_to_string(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

fn apply_coverage_excludes(report: &mut CoverageReport, excludes: &[String], root: &Path) {
    if excludes.is_empty() {
        return;
    }
    let Ok(set) = build_coverage_globset(excludes) else {
        return;
    };
    report.files.retain(|f| {
        // Match against the path relative to the scan root.
        let rel = f.path.strip_prefix(root).unwrap_or(&f.path);
        let rel = rel.to_string_lossy().replace('\\', "/");
        !set.is_match(&rel)
    });
    report.recompute_totals();
}

fn build_coverage_globset(patterns: &[String]) -> anyhow::Result<globset::GlobSet> {
    let mut b = globset::GlobSetBuilder::new();
    for p in patterns {
        let glob = if p.contains('*') || p.contains('?') || p.contains('[') {
            globset::Glob::new(p)?
        } else {
            globset::Glob::new(&format!("**/{}**", p))?
        };
        b.add(glob);
    }
    Ok(b.build()?)
}

fn run_analyzer(files: &[PathBuf], cfg: &AnalyzeConfig, args: &ScanArgs) -> ProjectAnalysis {
    // --- Incremental scanning (Phase B) ---
    // If a previous state exists, skip files whose hash matches.
    let prev = crate::state::Snapshot::load(
        &args
            .path
            .canonicalize()
            .unwrap_or_else(|_| args.path.clone()),
    );
    let scan_root = args
        .path
        .canonicalize()
        .unwrap_or_else(|_| args.path.clone());
    let (changed, cached): (Vec<PathBuf>, Vec<FileAnalysis>) = if prev.files.is_empty() {
        (files.to_vec(), vec![])
    } else {
        let mut ch = Vec::new();
        let mut ca = Vec::new();
        for f in files {
            let normalized = crate::util::path::normalize(f);
            let s = normalized.to_string_lossy().to_string();
            let scan_str = crate::util::path::normalize(&scan_root)
                .to_string_lossy()
                .to_string();
            let rel = if s.len() > scan_str.len()
                && s[..scan_str.len()].eq_ignore_ascii_case(&scan_str)
            {
                let mut rest = &s[scan_str.len()..];
                while rest.starts_with('\\') || rest.starts_with('/') {
                    rest = &rest[1..];
                }
                rest.replace('\\', "/")
            } else {
                s.replace('\\', "/")
            };
            let cur_hash = crate::state::Snapshot::hash_file(f).unwrap_or_default();
            if let Some(snap) = prev.files.get(&rel) {
                if snap.hash == cur_hash {
                    // File unchanged — reconstruct from state.
                    // Tokenize for duplication detection (cheap, no parsing).
                    let (tokens, nosonar_count) = std::fs::read_to_string(f)
                        .ok()
                        .map(|c| {
                            let lang = crate::scanner::language::detect(f);
                            (
                                Some(crate::analyzer::tokenize::tokenize(&c)),
                                crate::scanner::nosonar::count(&c, lang),
                            )
                        })
                        .unwrap_or((None, 0));
                    ca.push(FileAnalysis {
                        path: f.clone(),
                        language: crate::scanner::language::detect(f),
                        analyzed: false,
                        metrics: None,
                        tokens,
                        nosonar_count,
                        issues: snap
                            .issues
                            .iter()
                            .map(|ti| Issue {
                                rule_id: ti.rule_id.clone(),
                                severity: Severity::Info, // best-effort
                                message: ti.message.clone(),
                                file: f.clone(),
                                start_line: ti.line,
                                end_line: ti.line,
                                start_column: 0,
                                end_column: 0,
                            })
                            .collect(),
                    });
                    continue;
                }
            }
            ch.push(f.clone());
        }
        (ch, ca)
    };
    let skipped = cached.len();
    let total = files.len();

    let show_progress = !args.quiet
        && matches!(args.format, Format::Terminal)
        && changed.len() > 100
        && std::io::stderr().is_terminal();

    let mut result = if show_progress {
        let pb = indicatif::ProgressBar::new(changed.len() as u64);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:30.cyan/blue}] {pos}/{len} files")
                .expect("valid progress template")
                .progress_chars("█▓▒░"),
        );
        let cnt = changed.len() as u64;
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let anim = std::thread::spawn(move || {
            let mut i = 0u64;
            while rx.try_recv().is_err() {
                pb.set_message("analyzing…".to_string());
                pb.set_position(i.min(cnt));
                std::thread::sleep(std::time::Duration::from_millis(50));
                i = i.saturating_add(1);
            }
            pb.finish_and_clear();
        });
        let res = analyzer::analyze(&changed, cfg);
        let _ = tx.send(());
        let _ = anim.join();
        res
    } else {
        analyzer::analyze(&changed, cfg)
    };

    // Merge cached (unchanged) file analyses back in.
    result.files.extend(cached);

    // Re-run duplication detection across ALL files (changed + cached).
    // The analyzer only saw `changed` files, but duplication needs the full set.
    if skipped > 0 {
        let tokens: Vec<(PathBuf, Vec<crate::analyzer::tokenize::Token>)> = result
            .files
            .iter()
            .filter(|a| {
                a.path
                    .to_str()
                    .map_or(true, |p| !crate::analyzer::is_test_or_generated_file(p))
            })
            .filter_map(|a| {
                let toks = a.tokens.as_ref()?;
                Some((a.path.clone(), toks.clone()))
            })
            .collect();
        result.duplication = crate::analyzer::duplication::detect_with_mode(
            &tokens,
            cfg.duplication_mode,
            cfg.k_shingle,
            cfg.winnow_window,
            cfg.min_duplicate_tokens,
            cfg.min_duplicate_lines,
            cfg.normalize_identifiers,
        );
    }

    if skipped > 0 && !args.quiet {
        println!(
            "  {} scanned {} of {} files ({} skipped, hash match)",
            "⚡".yellow(),
            changed.len(),
            total,
            skipped
        );
    }

    result
}

fn evaluate_gate(
    analysis: &ProjectAnalysis,
    coverage: &CoverageReport,
    fail_above_percent: f64,
    fail_below_percent: f64,
    max_rating: Option<crate::rating::Rating>,
) -> bool {
    let mut all_pass = true;
    let mut messages: Vec<(bool, String)> = Vec::new();

    // Quality ratings (Reliability / Security / Maintainability).
    let (rel, sec, maint) = crate::rating::compute_ratings(&analysis.files);
    if let Some(max) = max_rating {
        for (name, r) in [
            ("reliability", rel),
            ("security", sec),
            ("maintainability", maint),
        ] {
            if rating_worse(r, max) {
                messages.push((
                    false,
                    format!("{} rating {} > max {}", name, r.as_str(), max.as_str()),
                ));
                all_pass = false;
            } else {
                messages.push((
                    true,
                    format!("{} rating {} ≤ max {}", name, r.as_str(), max.as_str()),
                ));
            }
        }
    }

    // Duplication check.
    let dup = &analysis.duplication;
    if dup.duplication_percent > fail_above_percent {
        messages.push((
            false,
            format!(
                "duplication {:.2}% > {:.2}%",
                dup.duplication_percent, fail_above_percent
            ),
        ));
        all_pass = false;
    } else {
        messages.push((
            true,
            format!(
                "duplication {:.2}% ≤ {:.2}%",
                dup.duplication_percent, fail_above_percent
            ),
        ));
    }

    // Coverage check (only if a report was found).
    if !coverage.files.is_empty() {
        if coverage.coverage_percent < fail_below_percent {
            messages.push((
                false,
                format!(
                    "coverage {:.2}% < {:.2}%",
                    coverage.coverage_percent, fail_below_percent
                ),
            ));
            all_pass = false;
        } else {
            messages.push((
                true,
                format!(
                    "coverage {:.2}% ≥ {:.2}%",
                    coverage.coverage_percent, fail_below_percent
                ),
            ));
        }
        // New-code coverage (only if state was found).
        if coverage.new_total_lines > 0 {
            if coverage.new_coverage_percent < fail_below_percent {
                messages.push((
                    false,
                    format!(
                        "new_coverage {:.2}% < {:.2}%",
                        coverage.new_coverage_percent, fail_below_percent
                    ),
                ));
                all_pass = false;
            } else {
                messages.push((
                    true,
                    format!(
                        "new_coverage {:.2}% ≥ {:.2}%",
                        coverage.new_coverage_percent, fail_below_percent
                    ),
                ));
            }
        }
        // Unit-test coverage.
        if coverage.ut_total_lines() > 0 {
            messages.push((
                true,
                format!(
                    "ut_coverage {:.2}% (informational)",
                    coverage.ut_coverage_percent
                ),
            ));
        }
        // Integration-test coverage.
        if coverage.it_total_lines() > 0 {
            messages.push((
                true,
                format!(
                    "it_coverage {:.2}% (informational)",
                    coverage.it_coverage_percent
                ),
            ));
        }
    }

    // Issue checks (Phase 2). Default thresholds: blocker=0, critical=0,
    // major=-1 (no limit), minor=-1, info=-1. Override via [issues] in
    // quality-gate.toml (not yet wired into config — this is a sensible
    // default).
    let mut by_sev = [0usize; 5];
    for i in analysis.files.iter().flat_map(|a| a.issues.iter()) {
        match i.severity {
            Severity::Blocker => by_sev[0] += 1,
            Severity::Critical => by_sev[1] += 1,
            Severity::Major => by_sev[2] += 1,
            Severity::Minor => by_sev[3] += 1,
            Severity::Info => by_sev[4] += 1,
        }
    }
    let thresholds: [(Severity, &str, usize); 2] = [
        (Severity::Blocker, "blocker", 0),
        (Severity::Critical, "critical", 0),
    ];
    for (sev, label, max) in thresholds {
        let count = match sev {
            Severity::Blocker => by_sev[0],
            Severity::Critical => by_sev[1],
            _ => unreachable!(),
        };
        if count > max {
            messages.push((false, format!("{} {} > {} (max)", label, count, max)));
            all_pass = false;
        } else {
            messages.push((true, format!("{} {} ≤ {}", label, count, max)));
        }
    }

    if all_pass {
        println!(
            "{} {}",
            "✓ Quality gate: PASS".green().bold(),
            format!(
                "({})",
                messages
                    .iter()
                    .map(|(_, m)| m.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .dimmed()
        );
    } else {
        println!(
            "{} {}",
            "✗ Quality gate: FAIL".red().bold(),
            format!(
                "({})",
                messages
                    .iter()
                    .map(|(_, m)| m.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .dimmed()
        );
    }
    all_pass
}

fn print_language_breakdown(files: &[PathBuf]) {
    let mut totals: std::collections::HashMap<language::Language, usize> =
        std::collections::HashMap::new();
    for f in files {
        if let Some(lang) = language::detect(f) {
            *totals.entry(lang).or_default() += 1;
        }
    }
    let mut counts: Vec<_> = totals.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    for (lang, n) in counts {
        info!("  {}: {}", lang, n);
    }
}

// ---------------------------------------------------------------------------
// CI command
// ---------------------------------------------------------------------------

pub fn run_ci(config_arg: Option<PathBuf>, args: crate::cli::CiArgs) -> Result<ExitCode> {
    let scan_root = args
        .path
        .canonicalize()
        .unwrap_or_else(|_| args.path.clone());
    let config_path = Config::resolve_path(config_arg.as_deref(), &scan_root);
    let config = Config::load(config_path.as_deref())
        .with_context(|| format!("loading config from {:?}", config_path))?;

    let display_root = util::path::normalize(&scan_root);
    info!("CI scan root: {}", display_root.display());

    // Build scan args from ci args
    let scan_args = crate::cli::ScanArgs {
        path: args.path.clone(),
        format: crate::cli::Format::Sarif,
        output: Some(args.output.clone()),
        gate: args.gate,
        watch: false,
        verbose: false,
        no_gitignore: false,
        dry_run: false,
        quiet: true,
        no_color: true,
        token_mode: false,
        min_duplicate_lines: None,
        normalize_identifiers: false,
        coverage_ut: vec![],
        coverage_it: vec![],
        new_code: args.new_code,
        no_state: true,
        since_days: args.since_days,
        max_rating: args.max_rating,
    };

    // Run the scan
    let exit_code = run(config_arg, scan_args)?;

    // Print PR comment body if requested
    if args.pr_comment {
        print_pr_comment(&scan_root, &config, &args);
    }

    Ok(exit_code)
}

fn print_pr_comment(root: &Path, config: &Config, args: &crate::cli::CiArgs) {
    // Read the SARIF file to extract summary
    let sarif_path = &args.output;
    let sarif_content = match std::fs::read_to_string(sarif_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let sarif: serde_json::Value = match serde_json::from_str(&sarif_content) {
        Ok(v) => v,
        Err(_) => return,
    };

    let runs = sarif["runs"].as_array();
    let run = runs.and_then(|r| r.first());
    let results = run.and_then(|r| r["results"].as_array());
    let issue_count = results.map(|r| r.len()).unwrap_or(0);

    // Count by severity
    let mut errors = 0u64;
    let mut warnings = 0u64;
    let mut notes = 0u64;
    if let Some(results) = results {
        for r in results {
            match r["level"].as_str().unwrap_or("") {
                "error" => errors += 1,
                "warning" => warnings += 1,
                _ => notes += 1,
            }
        }
    }

    // Quality gate status
    let gate_passed = !args.gate || issue_count == 0;
    let gate_icon = if gate_passed { "✅" } else { "❌" };
    let gate_text = if gate_passed { "PASSED" } else { "FAILED" };

    println!("<!-- lens-ci-comment -->");
    println!("## 🔍 Lens CI Report");
    println!();
    println!("**Quality Gate:** {} {}", gate_icon, gate_text);
    println!();
    println!("| Metric | Value |");
    println!("|--------|-------|");
    println!(
        "| Issues | {} (🔴 {} · 🟡 {} · 🔵 {}) |",
        issue_count, errors, warnings, notes
    );

    // Show top 5 issues
    if issue_count > 0 {
        println!();
        println!("<details><summary>Top issues</summary>");
        println!();
        if let Some(results) = results {
            for (i, r) in results.iter().take(10).enumerate() {
                let rule = r["ruleId"].as_str().unwrap_or("?");
                let msg = r["message"]["text"].as_str().unwrap_or("");
                let loc = &r["locations"][0]["physicalLocation"];
                let file = loc["artifactLocation"]["uri"].as_str().unwrap_or("?");
                let line = loc["region"]["startLine"].as_u64().unwrap_or(0);
                let level = r["level"].as_str().unwrap_or("note");
                let icon = match level {
                    "error" => "🔴",
                    "warning" => "🟡",
                    _ => "🔵",
                };
                println!(
                    "{}. {} **{}**: {} ({}:{})",
                    i + 1,
                    icon,
                    rule,
                    msg,
                    file,
                    line
                );
            }
        }
        println!("</details>");
    }

    println!();
    println!("---");
    println!(
        "<sub>🤖 Generated by [Lens](https://github.com/fatmuh/lens) v{}</sub>",
        env!("CARGO_PKG_VERSION")
    );
}

// ---------------------------------------------------------------------------
// Terminal report
// ---------------------------------------------------------------------------

fn report_terminal(
    ctx: &ScanContext,
    display_config: &Option<PathBuf>,
    analysis: &ProjectAnalysis,
    coverage: &CoverageReport,
    nosonar_total: usize,
    duration: std::time::Duration,
    new_code: bool,
    since_days: Option<u32>,
) {
    use comfy_table::Table;

    let display_root = util::path::normalize(&ctx.root);
    let counts = language_counts(&ctx.files);

    println!();
    println!("{}", "  Lens scan results".bold().cyan());
    println!("  {}", "─".repeat(50).dimmed());
    println!("  Root:     {}", display_root.display());
    if let Some(p) = display_config {
        println!("  Config:   {}", p.display());
    } else {
        println!("  Config:   {}", "<defaults>".dimmed());
    }
    println!("  Files:    {}", ctx.files.len().to_string().bold());
    println!("  NOSONAR:  {} marker(s)", nosonar_total);
    println!("  Duration: {}", humanize_duration(duration));

    if !counts.is_empty() {
        let mut table = Table::new();
        table.set_header(vec!["Language", "Files"]);
        for (lang, n) in counts {
            table.add_row(vec![lang.to_string(), n.to_string()]);
        }
        println!("\n  By language:\n{table}");
    }

    if let Some(m) = &analysis.aggregate_metrics {
        print_metrics_summary(m);
    }

    print_duplication_summary(&analysis.duplication);
    print_coverage_summary(coverage);
    print_ratings_summary(&analysis.files);
    print_issues_summary(
        &analysis.files,
        &ctx.root,
        new_code,
        since_days,
        &ctx.config.significant_code,
    );

    println!();
    println!(
        "{}",
        "  ℹ Phase 1-6: Multi-language metrics, 105 built-in rules (TS/JS/Dart), security taint analysis, SonarQube-compatible duplication, and coverage parsing."
            .dimmed()
    );
    println!();
}

fn print_issues_summary(
    files: &[crate::analyzer::FileAnalysis],
    scan_root: &Path,
    new_code: bool,
    since_days: Option<u32>,
    sig_config: &crate::config::SignificantCodeConfig,
) {
    let snapshot = state::Snapshot::load(scan_root);
    // When --new-code is set, only NEW issues are shown; the rest are
    // hidden but the lifecycle counts still reflect the full picture.
    // --since-days filters by file mtime. The two combine with OR logic.
    let all: Vec<&Issue> = files
        .iter()
        .flat_map(|f| f.issues.iter())
        .filter(|i| {
            let new_match =
                new_code && matches!(snapshot.classify_issue(i), state::IssueStatus::New);
            let mtime_match = since_days.map_or(true, |d| state::modified_within_days(&i.file, d));
            match (new_code, since_days) {
                (true, Some(_)) => new_match || mtime_match,
                (true, None) => new_match,
                (false, Some(_)) => mtime_match,
                (false, None) => true,
            }
        })
        .collect();
    if all.is_empty() {
        if new_code || since_days.is_some() {
            println!("\n  {}", "Issues (filtered)".bold().cyan());
            println!("  {} no matching issues", "✓".green());
        }
        return;
    }
    let filtered = new_code || since_days.is_some();
    if filtered {
        let mut parts = vec!["Issues".to_string()];
        if new_code {
            parts.push("new code".into());
        }
        if let Some(d) = since_days {
            parts.push(format!("last {} days", d));
        }
        println!(
            "\n  {}",
            format!("{} ({})", parts[0], parts[1..].join(" + "))
                .bold()
                .cyan()
        );
        println!("  {} filter active — other issues hidden", "→".dimmed());
    } else {
        println!("\n  {}", "Issues".bold().cyan());
    }
    let mut by_sev = [0usize; 5];
    for i in &all {
        match i.severity {
            Severity::Blocker => by_sev[0] += 1,
            Severity::Critical => by_sev[1] += 1,
            Severity::Major => by_sev[2] += 1,
            Severity::Minor => by_sev[3] += 1,
            Severity::Info => by_sev[4] += 1,
        }
    }
    // Issue lifecycle tracking (NEW / PERSISTENT / FIXED).
    let mut new_count = 0usize;
    let mut persistent_count = 0usize;
    let mut fixed_count = 0usize;
    let mut regressed_count = 0usize;
    for i in &all {
        match snapshot.classify_issue(i) {
            state::IssueStatus::New => new_count += 1,
            state::IssueStatus::Persistent => persistent_count += 1,
            state::IssueStatus::Regressed => regressed_count += 1,
            state::IssueStatus::Fixed => {} // (not in current scan)
        }
    }
    // Fixed = issues from the previous snapshot that are no longer in current scan.
    let current_keys: std::collections::HashSet<String> =
        all.iter().map(|i| state::Snapshot::issue_key(i)).collect();
    for (_, prev_file) in &snapshot.files {
        for prev_issue in &prev_file.issues {
            if !current_keys.contains(&prev_issue.key) {
                // Only count as "fixed" if the file still exists and was unchanged or
                // had the same hash (otherwise the file changed and the issue may
                // simply have moved).
                // For simplicity, count any previous issue not in current as fixed.
                fixed_count += 1;
            }
        }
    }
    println!(
        "  {} blocker, {} critical, {} major, {} minor, {} info",
        by_sev[0].to_string().red().bold(),
        by_sev[1].to_string().red(),
        by_sev[2].to_string().yellow(),
        by_sev[3].to_string(),
        by_sev[4].to_string().dimmed(),
    );
    // Lifecycle: NEW / PERSISTENT / FIXED / REGRESSED.
    if !snapshot.files.is_empty() {
        println!(
            "  {} new, {} persistent, {} fixed, {} regressed",
            new_count.to_string().green().bold(),
            persistent_count,
            fixed_count.to_string().cyan(),
            regressed_count.to_string().magenta(),
        );
    }

    // Significant code breakdown.
    let scan_root_normalized = crate::util::path::normalize(scan_root)
        .to_string_lossy()
        .to_string()
        .replace('\\', "/");
    let sig_count: usize = all
        .iter()
        .filter(|i| {
            let normalized = crate::util::path::normalize(&i.file)
                .to_string_lossy()
                .to_string()
                .replace('\\', "/");
            let rel = if normalized.len() > scan_root_normalized.len()
                && normalized[..scan_root_normalized.len()]
                    .eq_ignore_ascii_case(&scan_root_normalized)
            {
                let mut rest = &normalized[scan_root_normalized.len()..];
                while rest.starts_with('/') {
                    rest = &rest[1..];
                }
                rest.to_string()
            } else {
                normalized
            };
            sig_config.is_significant(&rel)
        })
        .count();
    let non_sig = all.len() - sig_count;
    if non_sig > 0 {
        println!(
            "  {} in significant code, {} in test/generated (excluded from gate)",
            sig_count.to_string().green(),
            non_sig.to_string().dimmed(),
        );
    }

    // Top violated rules.
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for i in &all {
        *counts.entry(i.rule_id.as_str()).or_default() += 1;
    }
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\n  Top violated rules:");
    for (id, n) in sorted.iter().take(5) {
        println!("    {:>5}  {}", n.to_string().bold(), id.dimmed());
    }

    // Top 5 issues (highest severity first, then by location).
    let mut sorted_issues: Vec<&Issue> = all.clone();
    sorted_issues.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.start_line.cmp(&b.start_line))
    });
    println!(
        "\n  Top issues ({} of {} shown):",
        sorted_issues.len().min(10),
        sorted_issues.len()
    );
    for i in sorted_issues.iter().take(10) {
        let name = i.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        println!(
            "    {} [{:<8}] {}:{}-{}  {}",
            severity_glyph(i.severity),
            i.severity.as_str(),
            name.dimmed(),
            i.start_line,
            i.end_line.max(i.start_line),
            i.message,
        );
    }
}

fn severity_glyph(s: Severity) -> &'static str {
    match s {
        Severity::Blocker => "■",
        Severity::Critical => "■",
        Severity::Major => "■",
        Severity::Minor => "·",
        Severity::Info => "·",
    }
}

fn severity_to_sarif_level(s: Severity) -> &'static str {
    match s {
        Severity::Blocker | Severity::Critical => "error",
        Severity::Major => "warning",
        Severity::Minor | Severity::Info => "note",
    }
}

fn print_metrics_summary(m: &AggregateMetrics) {
    use comfy_table::{Cell, Table};
    // Use a generic label since we now support multiple languages
    println!("\n  {}", "Metrics".bold().cyan());
    let mut t = Table::new();
    t.set_header(vec![Cell::new("Metric"), Cell::new("Value")]);
    t.add_row(vec![Cell::new("Total LOC"), Cell::new(m.total_loc)]);
    t.add_row(vec![
        Cell::new("  code lines"),
        Cell::new(m.total_code_lines),
    ]);
    t.add_row(vec![
        Cell::new("  comment lines"),
        Cell::new(m.total_comment_lines),
    ]);
    t.add_row(vec![
        Cell::new("  blank lines"),
        Cell::new(m.total_blank_lines),
    ]);
    t.add_row(vec![Cell::new("Functions"), Cell::new(m.total_functions)]);
    t.add_row(vec![Cell::new("Classes"), Cell::new(m.total_classes)]);
    t.add_row(vec![Cell::new("Interfaces"), Cell::new(m.total_interfaces)]);
    t.add_row(vec![
        Cell::new("Type aliases"),
        Cell::new(m.total_type_aliases),
    ]);
    t.add_row(vec![Cell::new("Enums"), Cell::new(m.total_enums)]);
    t.add_row(vec![
        Cell::new("Total cyclomatic complexity"),
        Cell::new(m.total_complexity),
    ]);
    t.add_row(vec![
        Cell::new("Avg complexity / function"),
        Cell::new(format!("{:.2}", m.avg_complexity_per_function)),
    ]);
    println!("{t}");

    if let Some(f) = &m.max_function {
        println!(
            "  {} Most complex: {}() (CC={}, line {})",
            "🔥".bold(),
            f.name.bold(),
            f.complexity,
            f.start_line
        );
    }
}

fn print_coverage_summary(c: &CoverageReport) {
    if c.files.is_empty() {
        return;
    }
    println!("\n  {}", "Coverage".bold().cyan());
    let color = if c.coverage_percent >= 80.0 {
        "green"
    } else if c.coverage_percent >= 50.0 {
        "yellow"
    } else {
        "red"
    };
    let pct_str = format!("{:.2}%", c.coverage_percent);
    let colored = match color {
        "green" => pct_str.green().to_string(),
        "yellow" => pct_str.yellow().to_string(),
        _ => pct_str.red().to_string(),
    };
    println!(
        "  {} of {} executable lines covered across {} file(s) [format: {}]",
        colored, c.total_lines, c.file_count, c.format
    );
    if c.new_total_lines > 0 {
        let nc = if c.new_coverage_percent >= 80.0 {
            "green"
        } else if c.new_coverage_percent >= 50.0 {
            "yellow"
        } else {
            "red"
        };
        let nc_str = format!("{:.2}%", c.new_coverage_percent);
        let nc_colored = match nc {
            "green" => nc_str.green().to_string(),
            "yellow" => nc_str.yellow().to_string(),
            _ => nc_str.red().to_string(),
        };
        println!(
            "  New code: {} of {} lines covered ({})",
            nc_colored,
            c.new_total_lines,
            format!("{}/{}", c.new_covered_lines, c.new_total_lines).dimmed()
        );
    }
    let mut low: Vec<&crate::coverage::FileCoverage> = c
        .files
        .iter()
        .filter(|f| f.coverage_percent < 100.0)
        .collect();
    low.sort_by(|a, b| {
        a.coverage_percent
            .partial_cmp(&b.coverage_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if !low.is_empty() {
        println!("\n  Top 5 files with lowest coverage:");
        for f in low.iter().take(5) {
            let name = f.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            println!(
                "    {:6.2}%  {} ({} uncovered)",
                f.coverage_percent,
                name.dimmed(),
                f.uncovered_lines.len()
            );
        }
    }
}

fn rating_worse(actual: crate::rating::Rating, max: crate::rating::Rating) -> bool {
    use crate::rating::Rating::*;
    let order = [A, B, C, D, E];
    let a = order.iter().position(|r| *r == actual).unwrap_or(0);
    let m = order.iter().position(|r| *r == max).unwrap_or(0);
    a > m
}

fn print_ratings_summary(files: &[crate::analyzer::FileAnalysis]) {
    let (rel, sec, maint) = crate::rating::compute_ratings(files);
    println!("\n  {}", "Quality ratings".bold().cyan());
    let p = |r: crate::rating::Rating| format!("{}{}{}", r.ansi(), r.as_str(), "\x1b[0m");
    println!(
        "  Reliability:       {}    (files with Blocker or Critical issues)",
        p(rel)
    );
    println!(
        "  Security:          {}    (files with Blocker issues)",
        p(sec)
    );
    println!(
        "  Maintainability:   {}    (files with Major+ issues)",
        p(maint)
    );
}

fn print_duplication_summary(d: &DuplicationReport) {
    println!("\n  {}", "Duplication".bold().cyan());
    let color = if d.duplication_percent > 3.0 {
        "red"
    } else if d.duplication_percent > 1.0 {
        "yellow"
    } else {
        "green"
    };
    let pct_str = format!("{:.2}%", d.duplication_percent);
    let colored = match color {
        "red" => pct_str.red().to_string(),
        "yellow" => pct_str.yellow().to_string(),
        _ => pct_str.green().to_string(),
    };
    let (label, unit) = match d.mode {
        crate::analyzer::duplication::DuplicationMode::Token => ("token-based", "tokens"),
        crate::analyzer::duplication::DuplicationMode::Sonar => {
            ("sonar-compat (line-based)", "lines")
        }
    };
    let fingerprint_note = if d.shared_fingerprint_count > 0 {
        format!(" ({} shared fingerprint(s))", d.shared_fingerprint_count)
    } else {
        String::new()
    };
    println!(
        "  {}: {} of {} {} are duplicated{}",
        label, colored, d.total_tokens, unit, fingerprint_note
    );
    if !d.top_offenders.is_empty() {
        println!("\n  Top duplicated files:");
        for (path, count) in d.top_offenders.iter().take(5) {
            let rel = path
                .strip_prefix(&path.ancestors().nth(1).unwrap_or(path))
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            println!("    {} tokens dup  {}", count, rel.dimmed());
        }
    }

    if !d.blocks.is_empty() {
        println!("\n  Top duplicate blocks:");
        for block in d.blocks.iter().take(5) {
            println!("    {} tokens", block.token_count.to_string().bold());
            for occ in &block.occurrences {
                let name = occ.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                println!(
                    "      {}:{}-{}",
                    name.dimmed(),
                    occ.start_line,
                    occ.end_line
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JSON report (forward-compatible schema, now with real metrics + duplication)
// ---------------------------------------------------------------------------

fn report_json(
    ctx: &ScanContext,
    display_config: &Option<PathBuf>,
    analysis: &ProjectAnalysis,
    coverage: &CoverageReport,
    duration_ms: u64,
    output: Option<&PathBuf>,
) -> Result<()> {
    let display_root = util::path::normalize(&ctx.root);
    let counts = language_counts(&ctx.files);
    let by_lang: BTreeMap<String, usize> = counts
        .into_iter()
        .map(|(lang, n)| (lang.to_string(), n))
        .collect();

    let nosonar: Vec<_> = analysis
        .files
        .iter()
        .filter(|a| a.nosonar_count > 0)
        .map(|a| {
            let rel = a.path.strip_prefix(&ctx.root).unwrap_or(&a.path);
            let rel = rel.to_string_lossy().replace('\\', "/");
            serde_json::json!({ "file": rel, "count": a.nosonar_count })
        })
        .collect();

    // Metrics.
    let metrics_json = if let Some(m) = &analysis.aggregate_metrics {
        serde_json::json!({
            "total_files": m.total_files,
            "total_loc": m.total_loc,
            "code_lines": m.total_code_lines,
            "comment_lines": m.total_comment_lines,
            "blank_lines": m.total_blank_lines,
            "comment_density": if m.total_code_lines + m.total_comment_lines > 0 {
                m.total_comment_lines as f64 / (m.total_code_lines + m.total_comment_lines) as f64
            } else { 0.0 },
            "functions": m.total_functions,
            "classes": m.total_classes,
            "interfaces": m.total_interfaces,
            "type_aliases": m.total_type_aliases,
            "enums": m.total_enums,
            "cyclomatic_complexity": m.total_complexity,
            "avg_complexity_per_function": m.avg_complexity_per_function,
            "max_function": m.max_function.as_ref().map(|f| serde_json::json!({
                "name": f.name,
                "complexity": f.complexity,
                "start_line": f.start_line,
                "end_line": f.end_line,
                "parameter_count": f.parameter_count,
            })),
            "top_functions_by_complexity": m.top_functions.iter().map(|f| serde_json::json!({
                "name": f.name,
                "complexity": f.complexity,
                "start_line": f.start_line,
                "end_line": f.end_line,
                "parameter_count": f.parameter_count,
            })).collect::<Vec<_>>(),
        })
    } else {
        serde_json::Value::Null
    };

    // Duplication.
    let d = &analysis.duplication;
    let top_offenders: Vec<_> = d
        .top_offenders
        .iter()
        .map(|(p, n)| {
            let rel = p.strip_prefix(&ctx.root).unwrap_or(p);
            let rel = rel.to_string_lossy().replace('\\', "/");
            serde_json::json!({ "file": rel, "duplicated_tokens": n })
        })
        .collect();
    let blocks_json: Vec<_> = d
        .blocks
        .iter()
        .map(|b| {
            let occurrences: Vec<_> = b
                .occurrences
                .iter()
                .map(|o| {
                    let rel = o.file.strip_prefix(&ctx.root).unwrap_or(&o.file);
                    let rel = rel.to_string_lossy().replace('\\', "/");
                    serde_json::json!({
                        "file": rel,
                        "start_line": o.start_line,
                        "end_line": o.end_line,
                    })
                })
                .collect();
            serde_json::json!({
                "token_count": b.token_count,
                "occurrences": occurrences,
            })
        })
        .collect();
    // Coverage.
    let coverage_json = if coverage.files.is_empty() {
        serde_json::Value::Null
    } else {
        let top_low: Vec<_> = coverage
            .files
            .iter()
            .filter(|f| f.coverage_percent < 100.0)
            .map(|f| {
                let rel = f.path.strip_prefix(&ctx.root).unwrap_or(&f.path);
                let rel = rel.to_string_lossy().replace('\\', "/");
                serde_json::json!({
                    "file": rel,
                    "total_lines": f.total_lines,
                    "covered_lines": f.covered_lines,
                    "coverage_percent": f.coverage_percent,
                    "uncovered_lines": f.uncovered_lines.iter().take(50).copied().collect::<Vec<_>>(),
                })
            })
            .collect();
        serde_json::json!({
            "format": coverage.format,
            "file_count": coverage.file_count,
            "total_lines": coverage.total_lines,
            "covered_lines": coverage.covered_lines,
            "coverage_percent": coverage.coverage_percent,
            "files": top_low,
        })
    };

    let duplication_json = serde_json::json!({
        "total_tokens": d.total_tokens,
        "duplicated_tokens": d.duplicated_tokens,
        "duplication_percent": d.duplication_percent,
        "min_tokens_threshold": d.min_tokens_threshold,
        "k_shingle": d.k_shingle,
        "winnow_window": d.winnow_window,
        "files_with_duplication": d.files_with_duplication,
        "shared_fingerprint_count": d.shared_fingerprint_count,
        "top_offenders": top_offenders,
        "blocks": blocks_json,
    });

    // Issues (Phase 2).
    let all_issues: Vec<_> = analysis
        .files
        .iter()
        .flat_map(|a| a.issues.iter())
        .map(|i| {
            let rel = i.file.strip_prefix(&ctx.root).unwrap_or(&i.file);
            let rel = rel.to_string_lossy().replace('\\', "/");
            serde_json::json!({
                "rule_id": i.rule_id,
                "severity": i.severity.as_str(),
                "message": i.message,
                "file": rel,
                "start_line": i.start_line,
                "end_line": i.end_line,
                "start_column": i.start_column,
                "end_column": i.end_column,
            })
        })
        .collect();
    let issue_counts: BTreeMap<String, usize> = {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for i in analysis.files.iter().flat_map(|a| a.issues.iter()) {
            *m.entry(i.severity.as_str().to_string()).or_default() += 1;
        }
        m
    };

    let payload = serde_json::json!({
        "lens_version": env!("CARGO_PKG_VERSION"),
        "scan": {
            "root": display_root,
            "config": display_config,
            "duration_ms": duration_ms,
        },
        "summary": {
            "total_files": ctx.files.len(),
            "nosonar_markers": analysis.files.iter().map(|a| a.nosonar_count).sum::<usize>(),
            "by_language": by_lang,
            "issue_count": all_issues.len(),
            "issues_by_severity": issue_counts,
        },
        "nosonar_by_file": nosonar,
        "metrics": metrics_json,
        "duplication": duplication_json,
        "coverage": coverage_json,
        "issues": all_issues,
    });
    let json = serde_json::to_string_pretty(&payload)?;
    if let Some(p) = output {
        std::fs::write(p, &json).with_context(|| format!("writing JSON to {}", p.display()))?;
        println!("JSON report written to {}", p.display());
    } else {
        println!("{}", json);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// HTML report (now with metrics + duplication sections)
// ---------------------------------------------------------------------------

fn report_html(
    ctx: &ScanContext,
    display_config: &Option<PathBuf>,
    analysis: &ProjectAnalysis,
    coverage: &CoverageReport,
    nosonar_total: usize,
    duration: std::time::Duration,
    output: Option<&PathBuf>,
) -> Result<()> {
    let dir = output
        .cloned()
        .unwrap_or_else(|| ctx.config.html.output.clone());
    std::fs::create_dir_all(&dir).ok();
    let out = dir.join("index.html");
    let html = render_html(
        ctx,
        display_config,
        analysis,
        coverage,
        nosonar_total,
        duration,
    );
    std::fs::write(&out, html).with_context(|| format!("writing {}", out.display()))?;
    println!("HTML report written to {}", out.display());
    if ctx.config.html.open_browser {
        println!(
            "{}",
            "ℹ --open-browser not yet implemented (Phase 5).".dimmed()
        );
    }
    Ok(())
}

fn render_html(
    ctx: &ScanContext,
    display_config: &Option<PathBuf>,
    analysis: &ProjectAnalysis,
    coverage: &CoverageReport,
    nosonar_total: usize,
    duration: std::time::Duration,
) -> String {
    let display_root = util::path::normalize(&ctx.root);
    let counts = language_counts(&ctx.files);
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    let config_str = display_config
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<defaults>".into());

    let mut lang_rows = String::new();
    for (lang, n) in &counts {
        lang_rows.push_str(&format!(
            "<tr><td>{}</td><td class=\"num\">{}</td></tr>\n",
            lang, n
        ));
    }

    // Metrics section
    let metrics_html = if let Some(m) = &analysis.aggregate_metrics {
        let mut top_rows = String::new();
        for f in m.top_functions.iter().take(5) {
            top_rows.push_str(&format!(
                "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>\n",
                html_escape(&f.name),
                f.complexity,
                f.parameter_count,
                f.start_line
            ));
        }
        let comment_density = if m.total_code_lines + m.total_comment_lines > 0 {
            m.total_comment_lines as f64 / (m.total_code_lines + m.total_comment_lines) as f64
        } else {
            0.0
        };
        format!(
            r#"
  <section>
    <h2>📏 Metrics</h2>
    <div class="summary">
      <div class="stat"><div class="stat-value">{loc}</div><div class="stat-label">Total LOC</div></div>
      <div class="stat"><div class="stat-value">{funcs}</div><div class="stat-label">Functions</div></div>
      <div class="stat"><div class="stat-value">{classes}</div><div class="stat-label">Classes</div></div>
      <div class="stat"><div class="stat-value">{ifaces}</div><div class="stat-label">Interfaces</div></div>
      <div class="stat"><div class="stat-value">{cc}</div><div class="stat-label">Cyclomatic complexity</div></div>
      <div class="stat"><div class="stat-value">{avg:.2}</div><div class="stat-label">Avg CC / function</div></div>
      <div class="stat"><div class="stat-value">{cd:.0}%</div><div class="stat-label">Comment density</div></div>
    </div>
    <h3>Lines</h3>
    <table>
      <thead><tr><th>Category</th><th class="num">Lines</th></tr></thead>
      <tbody>
        <tr><td>Code</td><td class="num">{code}</td></tr>
        <tr><td>Comments</td><td class="num">{comments}</td></tr>
        <tr><td>Blank</td><td class="num">{blanks}</td></tr>
      </tbody>
    </table>
    <h3>Top 5 most complex functions</h3>
    <table>
      <thead><tr><th>Name</th><th class="num">CC</th><th class="num">Params</th><th class="num">Line</th></tr></thead>
      <tbody>{top_funcs}</tbody>
    </table>
  </section>
"#,
            loc = m.total_loc,
            funcs = m.total_functions,
            classes = m.total_classes,
            ifaces = m.total_interfaces,
            cc = m.total_complexity,
            avg = m.avg_complexity_per_function,
            cd = comment_density * 100.0,
            code = m.total_code_lines,
            comments = m.total_comment_lines,
            blanks = m.total_blank_lines,
            top_funcs = top_rows,
        )
    } else {
        r#"<section><h2>📏 Metrics</h2><p>No metrics computed (no supported languages found).</p></section>"#.to_string()
    };

    // Duplication section
    let d = &analysis.duplication;
    let mut dup_rows = String::new();
    for (p, n) in d.top_offenders.iter().take(5) {
        let rel = p.strip_prefix(&ctx.root).unwrap_or(p);
        let rel = rel.to_string_lossy().replace('\\', "/");
        dup_rows.push_str(&format!(
            "<tr><td>{}</td><td class=\"num\">{}</td></tr>\n",
            html_escape(&rel),
            n
        ));
    }

    // Top blocks table rows.
    let mut block_rows = String::new();
    for block in d.blocks.iter().take(10) {
        let occs: Vec<String> = block
            .occurrences
            .iter()
            .map(|o| {
                let rel = o.file.strip_prefix(&ctx.root).unwrap_or(&o.file);
                let rel = rel.to_string_lossy().replace('\\', "/");
                format!(
                    "<code>{}</code>:{}-{}",
                    html_escape(&rel),
                    o.start_line,
                    o.end_line
                )
            })
            .collect();
        block_rows.push_str(&format!(
            "<tr><td class=\"num\">{}</td><td>{}</td></tr>\n",
            block.token_count,
            occs.join("<br>")
        ));
    }
    let dup_color_class = if d.duplication_percent > 3.0 {
        "bad"
    } else if d.duplication_percent > 1.0 {
        "warn"
    } else {
        "good"
    };
    let duplication_html = format!(
        r#"
  <section>
    <h2>♻️ Duplication</h2>
    <div class="summary">
      <div class="stat"><div class="stat-value {cls}">{pct:.2}%</div><div class="stat-label">Duplication</div></div>
      <div class="stat"><div class="stat-value">{dup_tok}</div><div class="stat-label">Duplicated tokens</div></div>
      <div class="stat"><div class="stat-value">{total_tok}</div><div class="stat-label">Total tokens</div></div>
      <div class="stat"><div class="stat-value">{shared}</div><div class="stat-label">Shared fingerprints</div></div>
    </div>
    <p>Threshold: ≥ {min_tokens} tokens, k-gram = {k}, winnow window = {w}</p>
    <h3>Top 5 files with most duplication</h3>
    <table>
      <thead><tr><th>File</th><th class="num">Duplicated tokens</th></tr></thead>
      <tbody>{dup_rows}</tbody>
    </table>
    <h3>Top duplicate blocks</h3>
    <table>
      <thead><tr><th>Token count</th><th>Occurrences</th></tr></thead>
      <tbody>{block_rows}</tbody>
    </table>
  </section>
"#,
        cls = dup_color_class,
        pct = d.duplication_percent,
        dup_tok = d.duplicated_tokens,
        total_tok = d.total_tokens,
        shared = d.shared_fingerprint_count,
        min_tokens = d.min_tokens_threshold,
        k = d.k_shingle,
        w = d.winnow_window,
        dup_rows = dup_rows,
        block_rows = block_rows,
    );

    // Coverage section.
    let coverage_html = if coverage.files.is_empty() {
        String::new()
    } else {
        let cov_color_class = if coverage.coverage_percent >= 80.0 {
            "good"
        } else if coverage.coverage_percent >= 50.0 {
            "warn"
        } else {
            "bad"
        };
        let mut low_rows = String::new();
        let mut low: Vec<&crate::coverage::FileCoverage> = coverage
            .files
            .iter()
            .filter(|f| f.coverage_percent < 100.0)
            .collect();
        low.sort_by(|a, b| {
            a.coverage_percent
                .partial_cmp(&b.coverage_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for f in low.iter().take(10) {
            let rel = f.path.strip_prefix(&ctx.root).unwrap_or(&f.path);
            let rel = rel.to_string_lossy().replace('\\', "/");
            low_rows.push_str(&format!(
                "<tr><td><code>{}</code></td><td class=\"num\">{}/{}</td><td class=\"num\">{:.2}%</td></tr>\n",
                html_escape(&rel),
                f.covered_lines,
                f.total_lines,
                f.coverage_percent
            ));
        }
        format!(
            r#"
  <section>
    <h2>📈 Coverage</h2>
    <div class="summary">
      <div class="stat"><div class="stat-value {cls}">{pct:.2}%</div><div class="stat-label">Coverage</div></div>
      <div class="stat"><div class="stat-value">{covered}</div><div class="stat-label">Covered lines</div></div>
      <div class="stat"><div class="stat-value">{total}</div><div class="stat-label">Executable lines</div></div>
      <div class="stat"><div class="stat-value">{files}</div><div class="stat-label">Files</div></div>
    </div>
    <p>Format: <code>{fmt}</code></p>
    <h3>Top 10 files with lowest coverage</h3>
    <table>
      <thead><tr><th>File</th><th class="num">Covered / Total</th><th class="num">%</th></tr></thead>
      <tbody>{low_rows}</tbody>
    </table>
  </section>
"#,
            cls = cov_color_class,
            pct = coverage.coverage_percent,
            covered = coverage.covered_lines,
            total = coverage.total_lines,
            files = coverage.file_count,
            fmt = coverage.format,
            low_rows = low_rows,
        )
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Lens Report — {project}</title>
<style>
  * {{ box-sizing: border-box; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
         margin: 0; padding: 2rem; background: #f8fafc; color: #0f172a; }}
  header {{ max-width: 1100px; margin: 0 auto 2rem; }}
  h1 {{ color: #2563eb; margin: 0 0 0.5rem; }}
  h2 {{ margin-top: 0; color: #1e293b; }}
  h3 {{ color: #475569; font-size: 1rem; text-transform: uppercase; letter-spacing: 0.05em; }}
  .meta {{ color: #64748b; font-size: 0.9rem; }}
  .meta code {{ background: #e2e8f0; padding: 0.1rem 0.4rem; border-radius: 0.25rem; }}
  main {{ max-width: 1100px; margin: 0 auto; }}
  .summary {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
              gap: 1rem; margin-bottom: 2rem; }}
  .stat {{ background: white; padding: 1.25rem; border-radius: 0.5rem;
           box-shadow: 0 1px 3px rgba(0,0,0,0.05); }}
  .stat-value {{ font-size: 2rem; font-weight: 700; color: #1e293b; }}
  .stat-value.good {{ color: #16a34a; }}
  .stat-value.warn {{ color: #d97706; }}
  .stat-value.bad {{ color: #dc2626; }}
  .stat-label {{ color: #64748b; font-size: 0.875rem; text-transform: uppercase;
                 letter-spacing: 0.05em; margin-top: 0.25rem; }}
  section {{ background: white; padding: 1.5rem; border-radius: 0.5rem;
             box-shadow: 0 1px 3px rgba(0,0,0,0.05); margin-bottom: 1rem; }}
  table {{ width: 100%; border-collapse: collapse; margin-top: 0.5rem; }}
  th, td {{ padding: 0.5rem 0.75rem; text-align: left; border-bottom: 1px solid #e2e8f0; }}
  th {{ background: #f1f5f9; font-weight: 600; }}
  td.num {{ text-align: right; font-variant-numeric: tabular-nums; }}
  footer {{ max-width: 1100px; margin: 2rem auto 0; color: #94a3b8; font-size: 0.875rem; }}
  .phase-badge {{ display: inline-block; background: #dbeafe; color: #1e40af;
                  padding: 0.15rem 0.5rem; border-radius: 1rem; font-size: 0.75rem;
                  font-weight: 600; }}
</style>
</head>
<body>
<header>
  <h1>📊 Lens Report</h1>
  <div class="meta">
    <div>Project: <code>{project}</code></div>
    <div>Config:  <code>{config}</code></div>
    <div>Generated: {timestamp} · Lens {version} · <span class="phase-badge">Phase 1</span></div>
  </div>
</header>
<main>
  <div class="summary">
    <div class="stat"><div class="stat-value">{total_files}</div><div class="stat-label">Files scanned</div></div>
    <div class="stat"><div class="stat-value">{nosonar}</div><div class="stat-label">NOSONAR markers</div></div>
    <div class="stat"><div class="stat-value">{duration}</div><div class="stat-label">Duration</div></div>
  </div>

  <section>
    <h2>Files by language</h2>
    <table>
      <thead><tr><th>Language</th><th class="num">Files</th></tr></thead>
      <tbody>
        {rows}
      </tbody>
    </table>
  </section>

  {metrics}
  {duplication}
  {coverage_html}

  <section>
    <h2>What&apos;s next</h2>
    <p>Lens 0.1.0 ships Phase 1+3 (TypeScript-only metrics, token-level
       duplication with block details, and coverage parsing for LCOV /
       Cobertura / JaCoCo). Upcoming phases will add:</p>
    <ul>
      <li><strong>Phase 2</strong> — rule engine (clippy-style for Rust,
          pylint-style for Python, etc.)</li>
      <li><strong>Phase 3</strong> — coverage parsing (lcov, Cobertura, JaCoCo)
          + quality gate enforcement</li>
      <li><strong>Phase 4</strong> — HTML interactive (collapsible code
          blocks, file-by-file navigation)</li>
      <li><strong>Phase 5</strong> — expand to Rust, Python, Go, Java</li>
    </ul>
  </section>
</main>
<footer>
  Total {total} file{total_plural} across {languages} language{languages_plural}.
</footer>
</body>
</html>
"#,
        project = html_escape(&display_root.display().to_string()),
        config = html_escape(&config_str),
        timestamp = timestamp,
        version = env!("CARGO_PKG_VERSION"),
        total_files = ctx.files.len(),
        nosonar = nosonar_total,
        duration = humanize_duration(duration),
        rows = lang_rows,
        metrics = metrics_html,
        duplication = duplication_html,
        coverage_html = coverage_html,
        total = total,
        total_plural = if total == 1 { "" } else { "s" },
        languages = counts.len(),
        languages_plural = if counts.len() == 1 { "" } else { "s" },
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

// ---------------------------------------------------------------------------
// SARIF report (results still empty — issues arrive in Phase 2)
// ---------------------------------------------------------------------------

fn report_sarif(
    ctx: &ScanContext,
    analysis: &ProjectAnalysis,
    _nosonar_total: usize,
    output: Option<&PathBuf>,
) -> Result<()> {
    let display_root = util::path::normalize(&ctx.root);

    // Build a list of unique rules + the list of results, mirroring the
    // SARIF 2.1.0 schema.
    let all_issues: Vec<&Issue> = analysis
        .files
        .iter()
        .flat_map(|a| a.issues.iter())
        .collect();
    let mut rules_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut results: Vec<serde_json::Value> = Vec::new();
    for i in &all_issues {
        rules_map.entry(i.rule_id.clone()).or_insert_with(|| {
            serde_json::json!({
                "id": i.rule_id,
                "name": i.rule_id,
                "shortDescription": { "text": i.rule_id },
                "defaultConfiguration": { "level": severity_to_sarif_level(i.severity) },
            })
        });
        let rel = i.file.strip_prefix(&ctx.root).unwrap_or(&i.file);
        let rel = rel.to_string_lossy().replace('\\', "/");
        results.push(serde_json::json!({
            "ruleId": i.rule_id,
            "level": severity_to_sarif_level(i.severity),
            "message": { "text": i.message },
            "locations": [{
                "physicalLocation": {
                    "artifactLocation": { "uri": rel },
                    "region": {
                        "startLine": i.start_line,
                        "endLine": i.end_line.max(i.start_line),
                        "startColumn": i.start_column + 1,
                        "endColumn": i.end_column + 1,
                    }
                }
            }]
        }));
    }
    let rules: Vec<serde_json::Value> = rules_map.into_values().collect();

    let payload = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "lens",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/fatmuh/lens",
                    "rules": rules
                }
            },
            "results": results,
            "invocations": [{
                "executionSuccessful": true,
                "endTimeUtc": Utc::now().to_rfc3339(),
            }],
            "properties": {
                "lens": {
                    "root": display_root,
                    "totalFiles": ctx.files.len(),
                    "phase": "1+2"
                }
            }
        }]
    });
    let json = serde_json::to_string_pretty(&payload)?;
    if let Some(p) = output {
        std::fs::write(p, &json).with_context(|| format!("writing SARIF to {}", p.display()))?;
        println!("SARIF report written to {}", p.display());
    } else {
        println!("{}", json);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn language_counts(files: &[PathBuf]) -> Vec<(language::Language, usize)> {
    let mut counts: BTreeMap<language::Language, usize> = BTreeMap::new();
    for f in files {
        if let Some(lang) = language::detect(f) {
            *counts.entry(lang).or_default() += 1;
        }
    }
    counts.into_iter().collect()
}

fn humanize_duration(d: std::time::Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{} ms", ms)
    } else if ms < 60_000 {
        format!("{:.2} s", ms as f64 / 1000.0)
    } else {
        let secs = ms / 1000;
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{}m {}s", mins, secs)
    }
}
