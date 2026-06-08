//! Static analysis engine.
//!
//! Phase 1: TypeScript-only metrics + language-agnostic token-level
//! duplication detection. Designed so additional languages can be plugged
//! in by adding a new entry in `parser::get_language`.
//!
//! Phase 2: rule engine (see [`crate::rules`]) — runs every enabled rule
//! on each file and collects `Issue`s.

pub mod cognitive;
pub mod duplication;
pub mod metrics;
pub mod parser;
pub mod tokenize;

use std::path::PathBuf;

use rayon::prelude::*;

use crate::analyzer::duplication::DuplicationMode;
use crate::rules::{Issue, RuleRegistry};
use crate::scanner::language::{self, Language};

/// Configuration for the analysis run.
#[derive(Debug)]
#[allow(dead_code)]
pub struct AnalyzeConfig {
    /// Which duplication algorithm to use.
    pub duplication_mode: DuplicationMode,
    /// Minimum block size (in tokens) to consider a duplicate.
    /// Used when `duplication_mode` is `Token`.
    pub min_duplicate_tokens: usize,
    /// Minimum block size (in lines) to consider a duplicate.
    /// Used when `duplication_mode` is `Sonar`.
    pub min_duplicate_lines: usize,
    /// If true, identifiers are normalized to `@id` before line hashing in
    /// the SonarQube-compatible mode. This makes the algorithm invariant to
    /// variable/function renames — closer to SonarQube's behavior.
    pub normalize_identifiers: bool,
    /// k-gram size for shingling.
    pub k_shingle: usize,
    /// Window size for winnowing.
    pub winnow_window: usize,
    /// Minimum fingerprint count per file for metrics to be considered.
    pub min_file_size_for_complexity: usize,
    /// Rule registry to use for issue detection. If empty, rules are
    /// disabled.
    pub rules: RuleRegistry,
}

impl Default for AnalyzeConfig {
    fn default() -> Self {
        Self {
            duplication_mode: DuplicationMode::Token,
            min_duplicate_tokens: 100,
            min_duplicate_lines: 100,
            normalize_identifiers: false,
            k_shingle: 5,
            winnow_window: 10,
            min_file_size_for_complexity: 0,
            rules: RuleRegistry::default_registry(),
        }
    }
}

impl AnalyzeConfig {
    /// Create a config with per-rule thresholds from `quality-gate.toml`.
    pub fn with_rules_config(rules_cfg: &crate::config::RulesConfig) -> Self {
        let mut s = Self::default();
        s.rules = RuleRegistry::with_config(rules_cfg);
        s
    }
}

/// Per-file analysis result.
#[derive(Debug, Clone)]
pub struct FileAnalysis {
    pub path: PathBuf,
    /// Detected language (used for per-language metrics and the report).
    #[allow(dead_code)]
    pub language: Option<Language>,
    /// True for files that we successfully parsed and tokenized.
    #[allow(dead_code)]
    pub analyzed: bool,
    /// AST-based metrics (only for TypeScript/TSX).
    pub metrics: Option<metrics::FileMetrics>,
    /// Token stream (for duplication, only for analyzed files).
    pub tokens: Option<Vec<tokenize::Token>>,
    /// NOSONAR marker count (kept here so the report has a single source).
    pub nosonar_count: usize,
    /// Issues found by rules (Phase 2).
    pub issues: Vec<Issue>,
}

/// Project-wide analysis result.
pub struct ProjectAnalysis {
    pub files: Vec<FileAnalysis>,
    pub aggregate_metrics: Option<metrics::AggregateMetrics>,
    pub duplication: duplication::DuplicationReport,
}

/// Run analysis on a set of files (in parallel).
pub fn analyze(files: &[PathBuf], config: &AnalyzeConfig) -> ProjectAnalysis {
    // 1. Per-file analysis in parallel.
    let analyses: Vec<FileAnalysis> = files
        .par_iter()
        .map(|path| analyze_file(path, config))
        .collect();

    // 2. Aggregate metrics (only for files that have metrics).
    let aggregate_metrics = {
        let per_file: Vec<&metrics::FileMetrics> =
            analyses.iter().filter_map(|a| a.metrics.as_ref()).collect();
        if per_file.is_empty() {
            None
        } else {
            Some(metrics::aggregate(&per_file))
        }
    };

    // 3. Duplication detection across all analyzed files.
    let tokens: Vec<(PathBuf, Vec<tokenize::Token>)> = analyses
        .iter()
        .filter_map(|a| {
            let toks = a.tokens.clone()?;
            Some((a.path.clone(), toks))
        })
        .collect();

    let duplication = duplication::detect_with_mode(
        &tokens,
        config.duplication_mode,
        config.k_shingle,
        config.winnow_window,
        config.min_duplicate_tokens,
        config.min_duplicate_lines,
        config.normalize_identifiers,
    );

    ProjectAnalysis {
        files: analyses,
        aggregate_metrics,
        duplication,
    }
}

/// Analyze a single file: detect language, tokenize, parse for metrics,
/// count NOSONAR markers, run rules. Errors are non-fatal — we just skip
/// that file.
fn analyze_file(path: &PathBuf, config: &AnalyzeConfig) -> FileAnalysis {
    let lang = language::detect(path);

    // Read the file. If we can't read it, return an empty analysis.
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            return FileAnalysis {
                path: path.clone(),
                language: lang,
                analyzed: false,
                metrics: None,
                tokens: None,
                nosonar_count: 0,
                issues: Vec::new(),
            };
        }
    };

    // NOSONAR count (works on raw source, language-aware).
    let nosonar_count = crate::scanner::nosonar::count(&content, lang);

    // Tokenize for duplication (language-agnostic, strip comments + strings).
    let tokens = tokenize::tokenize(&content);

    // Metrics — only for TypeScript / TSX.
    let metrics = match lang {
        Some(Language::TypeScript) | Some(Language::Tsx) => {
            parser::with_parser(lang.unwrap(), &content, |tree| {
                metrics::compute(tree, &content, lang.unwrap())
            })
        }
        _ => None,
    };

    // Phase 2: run all enabled rules. Build a partial FileAnalysis for the
    // rules to consume (no `issues` field — that's what we're computing).
    let partial = FileAnalysis {
        path: path.clone(),
        language: lang,
        analyzed: true,
        metrics: metrics.clone(),
        tokens: Some(tokens.clone()),
        nosonar_count,
        issues: Vec::new(),
    };
    let issues = config.rules.run(&partial, &content);

    FileAnalysis {
        path: path.clone(),
        language: lang,
        analyzed: true,
        metrics,
        tokens: Some(tokens),
        nosonar_count,
        issues,
    }
}
