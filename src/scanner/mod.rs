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
    self, duplication::{DuplicationMode, DuplicationReport}, metrics::AggregateMetrics,
    AnalyzeConfig, ProjectAnalysis,
};
use crate::cli::{Format, ScanArgs};
use crate::config::Config;
use crate::coverage::{self, CoverageReport};
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
        Self { root, files, config, config_path }
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
    let duplication_mode = if args.sonar_compat {
        DuplicationMode::Sonar
    } else {
        DuplicationMode::parse(&config.duplication.mode).unwrap_or_default()
    };
    let analyze_cfg = AnalyzeConfig {
        duplication_mode,
        min_duplicate_tokens: config.duplication.min_tokens,
        min_duplicate_lines: args.min_duplicate_lines.unwrap_or(config.duplication.min_lines),
        ..AnalyzeConfig::default()
    };
    let analysis = run_analyzer(&files, &analyze_cfg, &args);
    let nosonar_total: usize = analysis.files.iter().map(|a| a.nosonar_count).sum();

    // Phase 3: parse coverage reports (if any). Missing files are silently
    // skipped.
    let coverage_paths: Vec<PathBuf> = config
        .coverage
        .report_paths
        .iter()
        .map(|p| scan_root.join(p))
        .collect();
    let mut coverage_report = coverage::parse_many(&coverage_paths);
    apply_coverage_excludes(&mut coverage_report, &config.coverage.exclude, &scan_root);

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
        Format::Sarif => report_sarif(&ctx, nosonar_total, args.output.as_ref())?,
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
        );
        if !gate_passed {
            return Ok(ExitCode::from(1));
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn apply_coverage_excludes(report: &mut CoverageReport, excludes: &[String], root: &Path) {
    if excludes.is_empty() {
        return;
    }
    let Ok(set) = build_coverage_globset(excludes) else { return };
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
    let show_progress = !args.quiet
        && matches!(args.format, Format::Terminal)
        && files.len() > 100
        && std::io::stderr().is_terminal();

    if show_progress {
        let pb = indicatif::ProgressBar::new(files.len() as u64);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:30.cyan/blue}] {pos}/{len} files")
                .expect("valid progress template")
                .progress_chars("█▓▒░"),
        );

        // Animate the progress bar in a separate thread while the analysis
        // runs. indicatif doesn't have a direct hook for rayon's iterators.
        let total = files.len() as u64;
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let anim = std::thread::spawn(move || {
            let mut i = 0u64;
            while rx.try_recv().is_err() {
                pb.set_message("analyzing…".to_string());
                pb.set_position(i.min(total));
                std::thread::sleep(std::time::Duration::from_millis(50));
                i = i.saturating_add(1);
            }
            pb.finish_and_clear();
        });
        let result = analyzer::analyze(files, cfg);
        let _ = tx.send(());
        let _ = anim.join();
        result
    } else {
        analyzer::analyze(files, cfg)
    }
}

fn evaluate_gate(
    analysis: &ProjectAnalysis,
    coverage: &CoverageReport,
    fail_above_percent: f64,
    fail_below_percent: f64,
) -> bool {
    let mut all_pass = true;
    let mut messages: Vec<(bool, String)> = Vec::new();

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
    }

    if all_pass {
        println!(
            "{} {}",
            "✓ Quality gate: PASS".green().bold(),
            format!("({})", messages.iter().map(|(_, m)| m.as_str()).collect::<Vec<_>>().join(", "))
                .dimmed()
        );
    } else {
        println!(
            "{} {}",
            "✗ Quality gate: FAIL".red().bold(),
            format!("({})", messages.iter().map(|(_, m)| m.as_str()).collect::<Vec<_>>().join(", "))
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
// Terminal report
// ---------------------------------------------------------------------------

fn report_terminal(
    ctx: &ScanContext,
    display_config: &Option<PathBuf>,
    analysis: &ProjectAnalysis,
    coverage: &CoverageReport,
    nosonar_total: usize,
    duration: std::time::Duration,
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

    println!();
    println!(
        "{}",
        "  ℹ Phase 1+3+5: TS-only metrics, block-level duplication, SonarQube-compatible mode, and coverage parsing (LCOV/Cobertura/JaCoCo)."
            .dimmed()
    );
    println!();
}

fn print_metrics_summary(m: &AggregateMetrics) {
    use comfy_table::{Cell, Table};
    println!("\n  {}", "Metrics (TypeScript)".bold().cyan());
    let mut t = Table::new();
    t.set_header(vec![Cell::new("Metric"), Cell::new("Value")]);
    t.add_row(vec![Cell::new("Total LOC"), Cell::new(m.total_loc)]);
    t.add_row(vec![Cell::new("  code lines"), Cell::new(m.total_code_lines)]);
    t.add_row(vec![Cell::new("  comment lines"), Cell::new(m.total_comment_lines)]);
    t.add_row(vec![Cell::new("  blank lines"), Cell::new(m.total_blank_lines)]);
    t.add_row(vec![Cell::new("Functions"), Cell::new(m.total_functions)]);
    t.add_row(vec![Cell::new("Classes"), Cell::new(m.total_classes)]);
    t.add_row(vec![Cell::new("Interfaces"), Cell::new(m.total_interfaces)]);
    t.add_row(vec![Cell::new("Type aliases"), Cell::new(m.total_type_aliases)]);
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
    let mut low: Vec<&crate::coverage::FileCoverage> = c
        .files
        .iter()
        .filter(|f| f.coverage_percent < 100.0)
        .collect();
    low.sort_by(|a, b| a.coverage_percent.partial_cmp(&b.coverage_percent).unwrap_or(std::cmp::Ordering::Equal));
    if !low.is_empty() {
        println!("\n  Top 5 files with lowest coverage:");
        for f in low.iter().take(5) {
            let name = f
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            println!(
                "    {:6.2}%  {} ({} uncovered)",
                f.coverage_percent,
                name.dimmed(),
                f.uncovered_lines.len()
            );
        }
    }
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
        crate::analyzer::duplication::DuplicationMode::Sonar => ("sonar-compat (line-based)", "lines"),
    };
    let fingerprint_note = if d.shared_fingerprint_count > 0 {
        format!(" ({} shared fingerprint(s))", d.shared_fingerprint_count)
    } else {
        String::new()
    };
    println!("  {}: {} of {} {} are duplicated{}",
        label, colored, d.total_tokens, unit, fingerprint_note);
    if !d.top_offenders.is_empty() {
        println!("\n  Top duplicated files:");
        for (path, count) in d.top_offenders.iter().take(5) {
            let rel = path.strip_prefix(&path.ancestors().nth(1).unwrap_or(path))
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
                let name = occ
                    .file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
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
        },
        "nosonar_by_file": nosonar,
        "metrics": metrics_json,
        "duplication": duplication_json,
        "coverage": coverage_json,
        "issues": [],       // Phase 2
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
    let dir = output.cloned().unwrap_or_else(|| ctx.config.html.output.clone());
    std::fs::create_dir_all(&dir).ok();
    let out = dir.join("index.html");
    let html = render_html(ctx, display_config, analysis, coverage, nosonar_total, duration);
    std::fs::write(&out, html).with_context(|| format!("writing {}", out.display()))?;
    println!("HTML report written to {}", out.display());
    if ctx.config.html.open_browser {
        println!("{}", "ℹ --open-browser not yet implemented (Phase 5).".dimmed());
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
    <h2>📏 Metrics (TypeScript)</h2>
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
    nosonar_total: usize,
    output: Option<&PathBuf>,
) -> Result<()> {
    let display_root = util::path::normalize(&ctx.root);
    let payload = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "lens",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/fatmuh/lens",
                    "rules": []
                }
            },
            "results": [],
            "invocations": [{
                "executionSuccessful": true,
                "endTimeUtc": Utc::now().to_rfc3339(),
            }],
            "properties": {
                "lens": {
                    "root": display_root,
                    "totalFiles": ctx.files.len(),
                    "nosonarMarkers": nosonar_total,
                    "phase": "1"
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
