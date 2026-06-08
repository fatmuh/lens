//! Coverage report parsing.
//!
//! Supports three common formats:
//! - **LCOV** (`.info` files produced by Jest, nyc, karma, ...)
//! - **Cobertura** (XML)
//! - **JaCoCo** (XML)

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
    pub executable_lines: Vec<u32>,
    pub covered_lines_set: std::collections::HashSet<u32>,
}

#[derive(Debug, Clone)]
pub struct CoverageReport {
    pub format: String,
    pub total_lines: u64,
    pub covered_lines: u64,
    pub coverage_percent: f64,
    pub file_count: usize,
    pub files: Vec<FileCoverage>,
    pub ut_lines: u64,
    pub ut_covered_lines: u64,
    pub ut_coverage_percent: f64,
    pub it_lines: u64,
    pub it_covered_lines: u64,
    pub it_coverage_percent: f64,
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
            ut_lines: 0, ut_covered_lines: 0, ut_coverage_percent: 0.0,
            it_lines: 0, it_covered_lines: 0, it_coverage_percent: 0.0,
            new_total_lines: 0, new_covered_lines: 0, new_coverage_percent: 0.0,
        }
    }

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

    /// Compute new-code coverage using git blame per-line (matching SonarQube).
    /// For Changed files, only counts lines whose commit is in the new code period.
    /// For Added files, counts all executable lines as new.
    pub fn compute_new_coverage(&mut self, snapshot: &crate::state::Snapshot, base_ref: Option<&str>) {
        use std::process::Command;

        let base_hash = match base_ref {
            Some(r) => r.to_string(),
            None => match Command::new("git").args(["merge-base", "origin/main", "HEAD"]).output() {
                Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
                _ => {
                    // Git unavailable: fall back to file-level counting.
                    self.fallback_file_level(snapshot);
                    return;
                }
            }
        };

        let new_commits: std::collections::HashSet<String> = match Command::new("git")
            .args(["rev-list", &base_hash, "..HEAD"]).output()
        {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
                .lines().map(|s| s.to_string()).collect(),
            _ => std::collections::HashSet::new(),
        };

        let mut new_total: u64 = 0;
        let mut new_covered: u64 = 0;
        let mut changed_files = 0usize;
        let mut blamed_files = 0usize;

        for f in &self.files {
            let rel = normalize_lcov_path(&f.path);
            let hash = crate::state::Snapshot::hash_file(&f.path).unwrap_or_default();
            let status = snapshot.classify_file(&rel, &hash);
            if !matches!(status, crate::state::FileStatus::Added | crate::state::FileStatus::Changed) {
                continue;
            }
            changed_files += 1;

            if matches!(status, crate::state::FileStatus::Added) {
                for line in &f.executable_lines {
                    new_total += 1;
                    if f.covered_lines_set.contains(line) { new_covered += 1; }
                }
                continue;
            }

            // Changed: use git blame -e -n
            let git_path = rel.replace('\\', "/");
            let blame = Command::new("git")
                .args(["blame", "-e", "-n", "HEAD", "--", &git_path])
                .output();
            match blame {
                Ok(o) if o.status.success() => {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    for bline in stdout.lines() {
                        if let Some(paren) = bline.find(')') {
                            let header = &bline[..paren];
                            if let Some(commit) = header.split_whitespace().next() {
                                if commit == "0000000000000000000000000000000000000000" { continue; }
                                if new_commits.contains(commit) {
                                    if let Some(lp) = header.split_whitespace().nth(1) {
                                        if let Ok(line_num) = lp.parse::<u32>() {
                                            new_total += 1;
                                            if f.covered_lines_set.contains(&line_num) { new_covered += 1; }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    for line in &f.executable_lines {
                        new_total += 1;
                        if f.covered_lines_set.contains(line) { new_covered += 1; }
                    }
                }
            }
        }

        self.new_total_lines = new_total;
        self.new_covered_lines = new_covered;
        self.new_coverage_percent = if new_total > 0 { (new_covered as f64 / new_total as f64) * 100.0 } else { 0.0 };
    }

    fn fallback_file_level(&mut self, snapshot: &crate::state::Snapshot) {
        self.new_total_lines = 0;
        self.new_covered_lines = 0;
        for f in &self.files {
            let rel = normalize_lcov_path(&f.path);
            let hash = crate::state::Snapshot::hash_file(&f.path).unwrap_or_default();
            if matches!(snapshot.classify_file(&rel, &hash), crate::state::FileStatus::Added | crate::state::FileStatus::Changed) {
                self.new_total_lines += f.total_lines;
                self.new_covered_lines += f.covered_lines;
            }
        }
        self.new_coverage_percent = if self.new_total_lines > 0 {
            (self.new_covered_lines as f64 / self.new_total_lines as f64) * 100.0
        } else { 0.0 };
    }

    pub fn ut_total_lines(&self) -> u64 { self.ut_lines }
    pub fn it_total_lines(&self) -> u64 { self.it_lines }
    pub fn recompute_totals(&mut self) {
        self.total_lines = self.files.iter().map(|f| f.total_lines).sum();
        self.covered_lines = self.files.iter().map(|f| f.covered_lines).sum();
        self.file_count = self.files.len();
        self.coverage_percent = if self.total_lines > 0 { (self.covered_lines as f64 / self.total_lines as f64) * 100.0 } else { 0.0 };
        for f in &mut self.files {
            f.coverage_percent = if f.total_lines > 0 { (f.covered_lines as f64 / f.total_lines as f64) * 100.0 } else { 0.0 };
        }
    }
}

fn normalize_lcov_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn detect_and_parse(path: &Path) -> Result<CoverageReport> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("reading coverage report {}: {}", path.display(), e))?;
    let trimmed = content.trim_start();
    if content.contains("end_of_record") { return Ok(lcov::parse(&content)); }
    if trimmed.starts_with("<?xml") || trimmed.starts_with('<') {
        if content.contains("<coverage") { return Ok(cobertura::parse(&content)); }
        if content.contains("<report") { return Ok(jacoco::parse(&content)); }
    }
    Err(anyhow!("could not detect coverage format in {}", path.display()))
}

pub fn parse_many(paths: &[PathBuf]) -> CoverageReport {
    let mut combined = CoverageReport::empty();
    for path in paths {
        if !path.exists() { continue; }
        match detect_and_parse(path) {
            Ok(report) => combined.merge(report),
            Err(e) => eprintln!("warning: {}: {}", path.display(), e),
        }
    }
    combined
}

pub fn parse_with_categories(overall_paths: &[PathBuf], ut_paths: &[PathBuf], it_paths: &[PathBuf]) -> (CoverageReport, CoverageReport, CoverageReport) {
    (parse_many(overall_paths), parse_many(ut_paths), parse_many(it_paths))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Snapshot, FileStatus};

    #[test]
    fn normalize_lcov_path_works() {
        let p = Path::new("src\\app\\foo.ts");
        assert_eq!(normalize_lcov_path(p), "src/app/foo.ts");
    }

    #[test]
    fn merge_sums_matching_files() {
        let mut a = CoverageReport {
            format: "lcov".into(), total_lines: 0, covered_lines: 0, coverage_percent: 0.0, file_count: 0,
            files: vec![FileCoverage { path: PathBuf::from("foo.ts"), total_lines: 10, covered_lines: 5, coverage_percent: 0.0, uncovered_lines: vec![2, 4], executable_lines: vec![1,2,3,4,5,6,7,8,9,10], covered_lines_set: [1,3,5,6,7,8,9,10].into() }],
            ut_lines: 0, ut_covered_lines: 0, ut_coverage_percent: 0.0,
            it_lines: 0, it_covered_lines: 0, it_coverage_percent: 0.0,
            new_total_lines: 0, new_covered_lines: 0, new_coverage_percent: 0.0,
        };
        let b = CoverageReport {
            format: "lcov".into(), total_lines: 0, covered_lines: 0, coverage_percent: 0.0, file_count: 0,
            files: vec![FileCoverage { path: PathBuf::from("foo.ts"), total_lines: 10, covered_lines: 7, coverage_percent: 0.0, uncovered_lines: vec![1, 2, 4], executable_lines: vec![1,2,3,4,5,6,7,8,9,10], covered_lines_set: [2,3,5,6,7,8,9,10].into() }],
            ut_lines: 0, ut_covered_lines: 0, ut_coverage_percent: 0.0,
            it_lines: 0, it_covered_lines: 0, it_coverage_percent: 0.0,
            new_total_lines: 0, new_covered_lines: 0, new_coverage_percent: 0.0,
        };
        a.merge(b);
        let foo = a.files.iter().find(|f| f.path == PathBuf::from("foo.ts")).unwrap();
        assert_eq!(foo.total_lines, 20);
        assert_eq!(foo.covered_lines, 12);
    }
}
