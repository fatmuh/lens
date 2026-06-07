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
    /// Unit-test-only coverage (if `--coverage-ut` was used).
    pub ut_lines: u64,
    pub ut_covered_lines: u64,
    pub ut_coverage_percent: f64,
    /// Integration-test-only coverage (if `--coverage-it` was used).
    pub it_lines: u64,
    pub it_covered_lines: u64,
    pub it_coverage_percent: f64,
    /// Coverage on files added/changed since the last scan (new code).
    /// Only populated if `.lens/state.json` exists.
    pub new_total_lines: u64,
    pub new_covered_lines: u64,
    pub new_coverage_percent: f64,
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
            ut_lines: 0,
            ut_covered_lines: 0,
            ut_coverage_percent: 0.0,
            it_lines: 0,
            it_covered_lines: 0,
            it_coverage_percent: 0.0,
            new_total_lines: 0,
            new_covered_lines: 0,
            new_coverage_percent: 0.0,
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

    /// Compute new-code coverage: for each file in `self`, if the file
    /// is added or changed vs `snapshot`, add its lines to the new-code
    /// totals. Files unchanged since last scan are NOT counted.
    pub fn compute_new_coverage(&mut self, snapshot: &crate::state::Snapshot) {
        let mut new_total: u64 = 0;
        let mut new_covered: u64 = 0;
        for f in &self.files {
            let rel = f.path.to_string_lossy().to_string();
            let hash = crate::state::Snapshot::hash_file(&f.path).unwrap_or_default();
            let status = snapshot.classify_file(&rel, &hash);
            if matches!(status, crate::state::FileStatus::Added | crate::state::FileStatus::Changed) {
                new_total += f.total_lines;
                new_covered += f.covered_lines;
            }
        }
        self.new_total_lines = new_total;
        self.new_covered_lines = new_covered;
        self.new_coverage_percent = if new_total > 0 {
            (new_covered as f64 / new_total as f64) * 100.0
        } else { 0.0 };
    }

    pub fn ut_total_lines(&self) -> u64 { self.ut_lines }
    pub fn it_total_lines(&self) -> u64 { self.it_lines }
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

/// Parse coverage paths, splitting them into "ut" and "it" categories.
/// Returns (overall, ut_report, it_report). The overall report has
/// `ut_*` and `it_*` fields populated for separate display.
pub fn parse_with_categories(
    overall_paths: &[PathBuf],
    ut_paths: &[PathBuf],
    it_paths: &[PathBuf],
) -> (CoverageReport, CoverageReport, CoverageReport) {
    let overall = parse_many(overall_paths);
    let ut = parse_many(ut_paths);
    let it = parse_many(it_paths);
    (overall, ut, it)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_coverage_only_counts_new_files() {
        // Create two files: one already in state (unchanged), one new.
        let dir = std::env::temp_dir().join("lens-newcov-test");
        std::fs::create_dir_all(&dir).unwrap();
        let old_file = dir.join("old.ts");
        let new_file = dir.join("new.ts");
        std::fs::write(&old_file, "x").unwrap();
        std::fs::write(&new_file, "y").unwrap();
        let old_hash = crate::state::Snapshot::hash_file(&old_file).unwrap();
        // Use the file's full path as the snapshot key (matches what
        // `f.path.to_string_lossy()` produces).
        let old_key = old_file.to_string_lossy().to_string();
        let mut snap = crate::state::Snapshot::default();
        snap.files.insert(old_key, crate::state::FileSnapshot {
            hash: old_hash,
            issues: vec![],
        });
        // Build a CoverageReport with both files, 4 lines each.
        let mut report = CoverageReport {
            format: "lcov".into(),
            total_lines: 8,
            covered_lines: 5,
            coverage_percent: 62.5,
            file_count: 2,
            files: vec![
                FileCoverage {
                    path: old_file.clone(),
                    total_lines: 4,
                    covered_lines: 4,  // 100% covered
                    coverage_percent: 100.0,
                    uncovered_lines: vec![],
                },
                FileCoverage {
                    path: new_file.clone(),
                    total_lines: 4,
                    covered_lines: 1,  // 25% covered
                    coverage_percent: 25.0,
                    uncovered_lines: vec![2, 3, 4],
                },
            ],
            ut_lines: 0, ut_covered_lines: 0, ut_coverage_percent: 0.0,
            it_lines: 0, it_covered_lines: 0, it_coverage_percent: 0.0,
            new_total_lines: 0, new_covered_lines: 0, new_coverage_percent: 0.0,
        };
        report.compute_new_coverage(&snap);
        // Only the new file should be counted (4 lines, 1 covered = 25%).
        assert_eq!(report.new_total_lines, 4);
        assert_eq!(report.new_covered_lines, 1);
        assert!((report.new_coverage_percent - 25.0).abs() < 0.01);
        std::fs::remove_dir_all(&dir).ok();
    }

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
            ut_lines: 0, ut_covered_lines: 0, ut_coverage_percent: 0.0,
            it_lines: 0, it_covered_lines: 0, it_coverage_percent: 0.0,
            new_total_lines: 0, new_covered_lines: 0, new_coverage_percent: 0.0,
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
            ut_lines: 0, ut_covered_lines: 0, ut_coverage_percent: 0.0,
            it_lines: 0, it_covered_lines: 0, it_coverage_percent: 0.0,
            new_total_lines: 0, new_covered_lines: 0, new_coverage_percent: 0.0,
        };
        a.merge(b);
        let foo = a.files.iter().find(|f| f.path == PathBuf::from("foo.ts")).unwrap();
        assert_eq!(foo.total_lines, 20);
        assert_eq!(foo.covered_lines, 12);
        assert_eq!(foo.uncovered_lines, vec![1, 2, 4]);
    }
}
