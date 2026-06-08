//! LCOV format parser.
//!
//! LCOV is a line-based text format used by Jest, nyc, karma, and many
//! other tools. Each record starts with `SF:<path>` and ends with
//! `end_of_record`. Between them, `DA:<line>,<count>` records give the
//! execution count for each instrumented line.
//!
//! Reference: <https://github.com/linux-test-project/lcov/blob/master/man/geninfo.1.php>

use std::path::PathBuf;

use crate::coverage::{CoverageReport, FileCoverage};

/// Parse an LCOV file's contents.
pub fn parse(content: &str) -> CoverageReport {
    let mut files: Vec<FileCoverage> = Vec::new();
    let mut current: Option<FileCoverage> = None;

    for raw in content.lines() {
        let line = raw.trim();
        if let Some(path) = line.strip_prefix("SF:") {
            // Start a new file record.
            if let Some(f) = current.take() {
                files.push(finalize(f));
            }
            current = Some(FileCoverage {
                path: normalize_path(path),
                total_lines: 0,
                covered_lines: 0,
                coverage_percent: 0.0,
                uncovered_lines: Vec::new(),
                executable_lines: Vec::new(),
                covered_lines_set: std::collections::HashSet::new(),
            });
        } else if let Some(rest) = line.strip_prefix("DA:") {
            // DA: <line>,<count>[,<checksum>]
            if let Some(f) = current.as_mut() {
                let mut parts = rest.splitn(3, ',');
                let line_num: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                let count: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                if line_num > 0 {
                    f.total_lines += 1;
                    f.executable_lines.push(line_num);
                    if count > 0 {
                        f.covered_lines += 1;
                        f.covered_lines_set.insert(line_num);
                    } else {
                        f.uncovered_lines.push(line_num);
                    }
                }
            }
        } else if line == "end_of_record" {
            if let Some(f) = current.take() {
                files.push(finalize(f));
            }
        }
        // All other lines (TN, FN, FNDA, FNF, FNH, BRDA, BRF, BRH, LF,
        // LH) are either function- or branch-level and we don't track
        // them in Phase 3. LF/LH would be redundant with DA-derived counts.
    }

    // If the file didn't end with end_of_record (rare), flush whatever's
    // open so we don't lose data.
    if let Some(f) = current.take() {
        files.push(finalize(f));
    }

    let mut report = CoverageReport {
        format: "lcov".into(),
        total_lines: 0,
        covered_lines: 0,
        coverage_percent: 0.0,
        file_count: 0,
        files,
        ut_lines: 0,
        ut_covered_lines: 0,
        ut_coverage_percent: 0.0,
        it_lines: 0,
        it_covered_lines: 0,
        it_coverage_percent: 0.0,
        new_total_lines: 0,
        new_covered_lines: 0,
        new_coverage_percent: 0.0,
    };
    report.recompute_totals();
    report
}

fn finalize(mut f: FileCoverage) -> FileCoverage {
    f.uncovered_lines.sort_unstable();
    f.uncovered_lines.dedup();
    f.coverage_percent = if f.total_lines > 0 {
        (f.covered_lines as f64 / f.total_lines as f64) * 100.0
    } else {
        0.0
    };
    f
}

/// LCOV paths sometimes use Windows backslashes; normalize to forward
/// slashes so they match scanned source files consistently.
fn normalize_path(s: &str) -> PathBuf {
    PathBuf::from(s.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_lcov() {
        let lcov = r#"SF:src/foo.ts
DA:1,1
DA:2,5
DA:3,0
DA:4,0
end_of_record
SF:src/bar.ts
DA:1,1
DA:2,1
end_of_record
"#;
        let r = parse(lcov);
        assert_eq!(r.file_count, 2);
        assert_eq!(r.total_lines, 6);
        assert_eq!(r.covered_lines, 4);
        assert!((r.coverage_percent - 66.666).abs() < 0.01);

        let foo = r
            .files
            .iter()
            .find(|f| f.path == PathBuf::from("src/foo.ts"))
            .unwrap();
        assert_eq!(foo.uncovered_lines, vec![3, 4]);
    }

    #[test]
    fn handles_windows_paths() {
        let lcov = "SF:src\\foo.ts\nDA:1,0\nend_of_record\n";
        let r = parse(lcov);
        assert_eq!(r.files[0].path, PathBuf::from("src/foo.ts"));
    }

    #[test]
    fn handles_missing_end_of_record() {
        let lcov = "SF:src/foo.ts\nDA:1,0\n";
        let r = parse(lcov);
        assert_eq!(r.file_count, 1);
        assert_eq!(r.covered_lines, 0);
    }
}
