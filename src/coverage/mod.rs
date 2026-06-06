//! Coverage report parsing.
//!
//! Supports three common formats:
//! - **LCOV** (`.info` files produced by Jest, nyc, karma, ...). This is
//!   the format `pos-glid-b2b` ships under `coverage/lcov.info`.
//! - **Cobertura** (XML; used by many Java/.NET tools).
//! - **JaCoCo** (XML; the default Java coverage tool).
//!
//! Use [`detect_and_parse`] when the format isn't known, or one of the
//! format-specific parsers directly.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub mod cobertura;
pub mod jacoco;
pub mod lcov;

#[derive(Debug, Clone)]
pub struct FileCoverage {
    pub path: PathBuf,
    pub total_lines: u64,
    pub covered_lines: u64,
    pub coverage_percent: f64,
    pub uncovered_lines: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct CoverageReport {
    pub format: String,
    pub total_lines: u64,
    pub covered_lines: u64,
    pub coverage_percent: f64,
    pub file_count: usize,
    pub files: Vec<FileCoverage>,
}

impl CoverageReport {
    pub fn empty() -> Self {
        Self {
            format: String::new(),
            total_lines: 0,
            covered_lines: 0,
            coverage_percent: 0.0,
            file_count: 0,
            files: vec![],
        }
    }

    /// Merge another report into this one. Files with matching paths have
    /// their counts summed; other files are appended.
    pub fn merge(&mut self, other: CoverageReport) {
        if self.format.is_empty() {
            self.format = other.format;
        }
        for f in other.files {
            if let Some(existing) = self.files.iter_mut().find(|e| e.path == f.path) {
                existing.total_lines += f.total_lines;
                existing.covered_lines += f.covered_lines;
                let mut uncovered: Vec<u32> =
                    existing.uncovered_lines.iter().chain(f.uncovered_lines.iter()).copied().collect();
                uncovered.sort_unstable();
                uncovered.dedup();
                existing.uncovered_lines = uncovered;
            } else {
                self.files.push(f);
            }
        }
        self.recompute_totals();
    }

    pub fn recompute_totals(&mut self) {
        self.total_lines = self.files.iter().map(|f| f.total_lines).sum();
        self.covered_lines = self.files.iter().map(|f| f.covered_lines).sum();
        self.file_count = self.files.len();
        self.coverage_percent = if self.total_lines > 0 {
            (self.covered_lines as f64 / self.total_lines as f64) * 100.0
        } else {
            0.0
        };
        for f in &mut self.files {
            f.coverage_percent = if f.total_lines > 0 {
                (f.covered_lines as f64 / f.total_lines as f64) * 100.0
            } else {
                0.0
            };
        }
    }
}

/// Try to detect and parse a coverage report from `path`. Returns an
/// error if the file cannot be read or its format is unrecognized.
pub fn detect_and_parse(path: &Path) -> Result<CoverageReport> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("reading coverage report {}: {}", path.display(), e))?;
    let trimmed = content.trim_start();

    // LCOV always contains `end_of_record` and line-based `DA:` lines.
    if content.contains("end_of_record") {
        return Ok(lcov::parse(&content));
    }

    // XML formats.
    if trimmed.starts_with("<?xml") || trimmed.starts_with('<') {
        // Cobertura: root element is <coverage ...>
        if content.contains("<coverage") {
            return Ok(cobertura::parse(&content));
        }
        // JaCoCo: root element is <report>
        if content.contains("<report") {
            return Ok(jacoco::parse(&content));
        }
    }

    Err(anyhow!(
        "could not detect coverage format in {} (not LCOV, Cobertura, or JaCoCo)",
        path.display()
    ))
}

/// Try to parse all coverage report files at the given paths. Missing
/// files are silently skipped (a common situation when a tool hasn't been
/// run yet). Returns an empty report if nothing was found.
pub fn parse_many(paths: &[PathBuf]) -> CoverageReport {
    let mut combined = CoverageReport::empty();
    for path in paths {
        if !path.exists() {
            continue;
        }
        match detect_and_parse(path) {
            Ok(report) => combined.merge(report),
            Err(e) => {
                eprintln!("warning: {}: {}", path.display(), e);
            }
        }
    }
    combined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_sums_matching_files() {
        let mut a = CoverageReport {
            format: "lcov".into(),
            total_lines: 0,
            covered_lines: 0,
            coverage_percent: 0.0,
            file_count: 0,
            files: vec![FileCoverage {
                path: PathBuf::from("foo.ts"),
                total_lines: 10,
                covered_lines: 5,
                coverage_percent: 0.0,
                uncovered_lines: vec![2, 4],
            }],
        };
        let b = CoverageReport {
            format: "lcov".into(),
            total_lines: 0,
            covered_lines: 0,
            coverage_percent: 0.0,
            file_count: 0,
            files: vec![FileCoverage {
                path: PathBuf::from("foo.ts"),
                total_lines: 10,
                covered_lines: 7,
                coverage_percent: 0.0,
                uncovered_lines: vec![1, 2, 4],
            }],
        };
        a.merge(b);
        let foo = a.files.iter().find(|f| f.path == PathBuf::from("foo.ts")).unwrap();
        assert_eq!(foo.total_lines, 20);
        assert_eq!(foo.covered_lines, 12);
        assert_eq!(foo.uncovered_lines, vec![1, 2, 4]);
    }
}
