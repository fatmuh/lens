//! Quality ratings (A–E) for Reliability, Security, Maintainability.
//!
//! These are Lens's own implementation of the standard 5-tier rating
//! system used by static-analysis tools. The mapping is:
//!
//! | Rating | % of files with at least one issue of that category |
//! |--------|------------------------------------------------------|
//! | A      | 0–5%                                                 |
//! | B      | 5–10%                                                |
//! | C      | 10–20%                                               |
//! | D      | 20–50%                                               |
//! | E      | 50–100%                                              |
//!
//! Categories:
//! - **Reliability**   — file has at least one `Blocker` or `Critical` issue.
//! - **Security**      — file has at least one `Blocker` issue (a stricter
//!                       bar; security regressions are usually blockers).
//! - **Maintainability** — file has at least one `Major`/`Critical`/`Blocker`
//!                       issue (debt-bearing severity).
//!
//! These are file-density ratings, not the technical-debt ratio SonarQube
//! uses (which needs per-rule remediation cost). This is the simpler form
//! that's still useful for a one-shot scan.

use serde::{Deserialize, Serialize};

use crate::analyzer::FileAnalysis;
use crate::rules::Severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rating {
    A,
    B,
    C,
    D,
    E,
}

impl Rating {
    /// Map a percentage (0-100) to a rating letter.
    pub fn from_percent(p: f64) -> Self {
        if p < 5.0 {
            Self::A
        } else if p < 10.0 {
            Self::B
        } else if p < 20.0 {
            Self::C
        } else if p < 50.0 {
            Self::D
        } else {
            Self::E
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
        }
    }

    /// ANSI color for terminal display.
    pub fn ansi(self) -> &'static str {
        match self {
            Self::A => "\x1b[32m", // green
            Self::B => "\x1b[36m", // cyan
            Self::C => "\x1b[33m", // yellow
            Self::D => "\x1b[31m", // red
            Self::E => "\x1b[35m", // magenta
        }
    }
}

/// Compute the three ratings for a project.
pub fn compute_ratings(files: &[FileAnalysis]) -> (Rating, Rating, Rating) {
    let total = files.len();
    if total == 0 {
        return (Rating::A, Rating::A, Rating::A);
    }
    let total_f = total as f64;
    let reliability = files
        .iter()
        .filter(|f| {
            f.issues
                .iter()
                .any(|i| matches!(i.severity, Severity::Blocker | Severity::Critical))
        })
        .count();
    let security = files
        .iter()
        .filter(|f| {
            f.issues
                .iter()
                .any(|i| matches!(i.severity, Severity::Blocker))
        })
        .count();
    let maintainability = files
        .iter()
        .filter(|f| {
            f.issues.iter().any(|i| {
                matches!(
                    i.severity,
                    Severity::Major | Severity::Critical | Severity::Blocker
                )
            })
        })
        .count();
    (
        Rating::from_percent((reliability as f64 / total_f) * 100.0),
        Rating::from_percent((security as f64 / total_f) * 100.0),
        Rating::from_percent((maintainability as f64 / total_f) * 100.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn file_with_severities(sevs: &[Severity]) -> FileAnalysis {
        FileAnalysis {
            path: PathBuf::from("test.ts"),
            language: Some(crate::scanner::language::Language::TypeScript),
            analyzed: true,
            metrics: None,
            tokens: None,
            nosonar_count: 0,
            issues: sevs
                .iter()
                .enumerate()
                .map(|(i, s)| crate::rules::Issue {
                    rule_id: format!("test-{}", i),
                    severity: *s,
                    message: "test".into(),
                    file: PathBuf::from("test.ts"),
                    start_line: 1,
                    end_line: 1,
                    start_column: 0,
                    end_column: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn rating_thresholds() {
        assert_eq!(Rating::from_percent(0.0), Rating::A);
        assert_eq!(Rating::from_percent(4.9), Rating::A);
        assert_eq!(Rating::from_percent(5.0), Rating::B);
        assert_eq!(Rating::from_percent(9.9), Rating::B);
        assert_eq!(Rating::from_percent(10.0), Rating::C);
        assert_eq!(Rating::from_percent(19.9), Rating::C);
        assert_eq!(Rating::from_percent(20.0), Rating::D);
        assert_eq!(Rating::from_percent(49.9), Rating::D);
        assert_eq!(Rating::from_percent(50.0), Rating::E);
        assert_eq!(Rating::from_percent(100.0), Rating::E);
    }

    #[test]
    fn empty_files_all_a() {
        let (r, s, m) = compute_ratings(&[]);
        assert_eq!(r, Rating::A);
        assert_eq!(s, Rating::A);
        assert_eq!(m, Rating::A);
    }

    #[test]
    fn one_critical_in_one_file() {
        // 1 file of 1 = 100% has critical → reliability E, security A, maintainability E
        let files = vec![file_with_severities(&[Severity::Critical])];
        let (r, s, m) = compute_ratings(&files);
        assert_eq!(r, Rating::E);
        assert_eq!(s, Rating::A);
        assert_eq!(m, Rating::E);
    }

    #[test]
    fn one_blocker_among_many() {
        // 1 file of 20 = 5% has blocker → reliability B, security B, maintainability E
        let mut files: Vec<FileAnalysis> = (0..19)
            .map(|_| file_with_severities(&[Severity::Major]))
            .collect();
        files.push(file_with_severities(&[Severity::Blocker]));
        let (r, s, m) = compute_ratings(&files);
        assert_eq!(r, Rating::B); // 1/20 = 5.0% → B
        assert_eq!(s, Rating::B); // 1/20 = 5.0% → B
        assert_eq!(m, Rating::E); // 20/20 = 100% → E
    }
}
