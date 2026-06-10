//! File discovery: walks the scan root, respects `.gitignore` and `.lensignore`,
//! and applies user-configured include/exclude globs.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

use crate::config::ScanConfig;

/// Walks `root` and returns the list of regular files that should be analyzed.
///
/// Rules, in order:
/// 1. Skip directories ignored by `.gitignore` (if `respect_gitignore`).
/// 2. Skip patterns from `.lensignore` (if `respect_lensignore`).
/// 3. Apply `[scan].include` whitelist (if non-empty, file MUST match).
/// 4. Apply `[scan].exclude` blacklist (file MUST NOT match).
/// 5. Skip files larger than `max_file_size_bytes`.
pub fn scan(root: &Path, cfg: &ScanConfig, no_gitignore: bool) -> Result<Vec<PathBuf>> {
    if !root.is_dir() {
        anyhow::bail!("scan root is not a directory: {}", root.display());
    }

    // Build glob sets.
    let include = build_globset(&cfg.include).context("building include globs")?;
    let exclude = build_globset(&cfg.exclude).context("building exclude globs")?;

    // Load .lensignore patterns from the configured file (or default name).
    let lensignore_patterns = if cfg.respect_lensignore {
        load_ignore_file(&root.join(&cfg.ignore_file))
    } else {
        Vec::new()
    };
    let lensignore_globs =
        build_globset(&lensignore_patterns).context("building .lensignore globs")?;

    let mut walker = WalkBuilder::new(root);
    walker
        .standard_filters(!no_gitignore)
        .require_git(false) // Don't require git repo to work
        .git_exclude(!no_gitignore)
        .git_ignore(!no_gitignore)
        .git_global(!no_gitignore)
        .hidden(false) // include dotfiles like .env, .eslintrc
        .add_custom_ignore_filename(".git")
        .add_custom_ignore_filename(".git");

    // Always exclude .git directory
    // (standard_filters should do this, but hidden(false) may override)
    let always_exclude = build_globset(&[
        ".git/**".to_string(),
        ".git".to_string(),
        "**/.git/**".to_string(),
    ])
    .ok()
    .flatten();

    // Inject custom ignore patterns via ignore's overlay mechanism.
    for _pat in &lensignore_patterns {
        walker.add_custom_ignore_filename(&cfg.ignore_file);
    }

    let mut files: Vec<PathBuf> = Vec::new();

    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!("walk error: {err}");
                continue;
            }
        };
        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }

        let path = entry.path().to_path_buf();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        // Include whitelist (if defined).
        if let Some(set) = &include {
            if !set.is_match(&rel) {
                continue;
            }
        }

        // Exclude blacklist.
        if let Some(set) = &exclude {
            if set.is_match(&rel) {
                continue;
            }
        }
        if let Some(set) = &always_exclude {
            if set.is_match(&rel) {
                continue;
            }
        }
        if let Some(set) = &lensignore_globs {
            if set.is_match(&rel) {
                continue;
            }
        }

        // File size cap.
        if let Ok(meta) = entry.metadata() {
            if meta.len() > cfg.max_file_size_bytes {
                tracing::debug!("skip (too large): {}", path.display());
                continue;
            }
        }

        // Only include files with recognized language extensions.
        // Skip .sample, .lock, .bin, and other non-source files.
        if crate::scanner::language::detect(&path).is_none() {
            continue;
        }

        // Exclude test files if configured
        if cfg.exclude_tests {
            let rel_norm = rel.replace('\\', "/");
            if crate::analyzer::is_test_or_generated_file(&rel_norm) {
                continue;
            }
        }

        files.push(path);
    }

    files.sort();
    Ok(files)
}

fn build_globset(patterns: &[String]) -> Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        // Allow plain strings as well as globs.
        let glob = if p.contains('*') || p.contains('?') || p.contains('[') {
            Glob::new(p)?
        } else {
            // Treat bare names as "anywhere in path".
            Glob::new(&format!("**/{}**", p))?
        };
        b.add(glob);
    }
    Ok(Some(b.build()?))
}

fn load_ignore_file(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}
