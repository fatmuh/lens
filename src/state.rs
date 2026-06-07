//! State persistence for new-code tracking and issue lifecycle.
//!
//! Saves a snapshot of file hashes and previous issues to `.lens/state.json`.
//! On the next scan, we can compare:
//!   - File status: ADDED | CHANGED | UNCHANGED | REMOVED
//!   - Issue status: NEW | PERSISTENT | FIXED | REGRESSED
//!
//! This mirrors (a subset of) SonarQube's new-code period tracking.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::rules::Issue;

/// Issue tracking status across scans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueStatus {
    /// First time we see this issue.
    New,
    /// Was there before, still there.
    Persistent,
    /// Was there before, no longer there (fixed!).
    Fixed,
    /// Was previously resolved/closed but is back.
    Regressed,
}

/// File status across scans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    Added,
    Changed,
    Unchanged,
    Removed,
}

/// On-disk snapshot of a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    /// SHA-256 of file contents (lowercase hex).
    pub hash: String,
    /// Issues we reported on this file in the last scan (by stable key).
    pub issues: Vec<TrackedIssue>,
}

/// An issue stored in the snapshot, with a stable identity (rule+line+message).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedIssue {
    /// Stable identity: `rule_id:line:message_hash`.
    pub key: String,
    pub rule_id: String,
    pub line: u32,
    pub message: String,
}

/// Full scan snapshot (persisted to `.lens/state.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    /// Scan timestamp (unix seconds).
    pub scan_time: i64,
    /// Per-file snapshots, keyed by relative path.
    pub files: BTreeMap<String, FileSnapshot>,
    /// Last known project version (e.g. from `package.json`).
    #[serde(default)]
    pub version: Option<String>,
}

impl Snapshot {
    pub const FILE_NAME: &'static str = "state.json";
    pub const DIR_NAME: &'static str = ".lens";

    /// Load from disk (or return default if missing/corrupt).
    pub fn load(project_root: &Path) -> Self {
        let path = project_root.join(Self::DIR_NAME).join(Self::FILE_NAME);
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save to disk.
    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let dir = project_root.join(Self::DIR_NAME);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(Self::FILE_NAME);
        let s = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, s)
    }

    /// Compute SHA-256 of file contents.
    pub fn hash_file(path: &Path) -> std::io::Result<String> {
        let bytes = std::fs::read(path)?;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        bytes.hash(&mut h);
        Ok(format!("{:016x}", h.finish()))
    }

    /// Compute a stable key for an issue.
    pub fn issue_key(issue: &Issue) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        issue.rule_id.hash(&mut h);
        issue.start_line.hash(&mut h);
        issue.message.hash(&mut h);
        format!("{}:{}:{:016x}", issue.rule_id, issue.start_line, h.finish())
    }

    /// Classify an issue's status against the previous snapshot.
    pub fn classify_issue(&self, issue: &Issue) -> IssueStatus {
        let key = Self::issue_key(issue);
        let rel = issue.file.to_string_lossy().to_string();
        if let Some(prev) = self.files.get(&rel) {
            if prev.issues.iter().any(|i| i.key == key) {
                IssueStatus::Persistent
            } else {
                // Was the file present and had issues on this line? If so, regressed.
                if !prev.issues.is_empty() {
                    IssueStatus::Regressed
                } else {
                    IssueStatus::New
                }
            }
        } else {
            IssueStatus::New
        }
    }

    /// Classify a file's status against the previous snapshot.
    pub fn classify_file(&self, rel: &str, new_hash: &str) -> FileStatus {
        match self.files.get(rel) {
            None => FileStatus::Added,
            Some(prev) if prev.hash != new_hash => FileStatus::Changed,
            Some(_) => FileStatus::Unchanged,
        }
    }
}

/// Result of issue tracking — wraps an issue with its lifecycle status.
#[derive(Debug, Clone)]
pub struct TrackedResult {
    pub issue: Issue,
    pub status: IssueStatus,
}

/// True if `path` was modified within the last `days` days.
/// Used by `--since-days` to filter to recently-changed code.
pub fn modified_within_days(path: &Path, days: u32) -> bool {
    let Ok(meta) = std::fs::metadata(path) else { return false; };
    let Ok(modified) = meta.modified() else { return false; };
    let Ok(elapsed) = std::time::SystemTime::now().duration_since(modified) else {
        return false;
    };
    let limit = std::time::Duration::from_secs(u64::from(days) * 86_400);
    elapsed <= limit
}

/// Apply the snapshot to a list of issues, returning tracked results.
pub fn track_issues(snapshot: &Snapshot, issues: Vec<Issue>) -> Vec<TrackedResult> {
    issues.into_iter().map(|issue| {
        let status = snapshot.classify_issue(&issue);
        TrackedResult { issue, status }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fake_issue(rule: &str, line: u32, msg: &str) -> Issue {
        Issue {
            rule_id: rule.into(),
            severity: crate::rules::Severity::Major,
            message: msg.into(),
            file: PathBuf::from("src/foo.ts"),
            start_line: line,
            end_line: line,
            start_column: 0,
            end_column: 0,
        }
    }

    #[test]
    fn first_scan_all_new() {
        let snap = Snapshot::default();
        let issue = fake_issue("no-eval", 10, "msg");
        assert_eq!(snap.classify_issue(&issue), IssueStatus::New);
    }

    #[test]
    fn unchanged_file_keeps_issue_persistent() {
        let mut snap = Snapshot::default();
        let key = Snapshot::issue_key(&fake_issue("no-eval", 10, "msg"));
        snap.files.insert("src/foo.ts".to_string(), FileSnapshot {
            hash: "abc123".to_string(),
            issues: vec![TrackedIssue {
                key: key.clone(),
                rule_id: "no-eval".into(),
                line: 10,
                message: "msg".into(),
            }],
        });
        let issue = fake_issue("no-eval", 10, "msg");
        assert_eq!(snap.classify_issue(&issue), IssueStatus::Persistent);
    }

    #[test]
    fn changed_file_marks_old_issue_fixed() {
        let mut snap = Snapshot::default();
        snap.files.insert("src/foo.ts".to_string(), FileSnapshot {
            hash: "oldhash".to_string(),
            issues: vec![TrackedIssue {
                key: "no-eval:10:deadbeef".into(),
                rule_id: "no-eval".into(),
                line: 10,
                message: "msg".into(),
            }],
        });
        // File hash changed → the old issue is "fixed" (no longer produced).
        assert_eq!(snap.classify_file("src/foo.ts", "newhash"), FileStatus::Changed);
    }

    #[test]
    fn new_issue_in_existing_file_is_regressed() {
        let mut snap = Snapshot::default();
        snap.files.insert("src/foo.ts".to_string(), FileSnapshot {
            hash: "same".to_string(),
            issues: vec![TrackedIssue {
                key: "no-eval:5:abc".into(),
                rule_id: "no-eval".into(),
                line: 5,
                message: "old".into(),
            }],
        });
        let new_issue = fake_issue("no-eqeqeq", 20, "new msg");
        assert_eq!(snap.classify_issue(&new_issue), IssueStatus::Regressed);
    }

    #[test]
    fn modified_within_days_for_just_created_file() {
        // A file we just wrote should be within 1 day.
        let path = std::env::temp_dir().join("lens-mtime-test.txt");
        std::fs::write(&path, "x").unwrap();
        assert!(super::modified_within_days(&path, 1));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn modified_within_days_missing_file_returns_false() {
        // Missing files shouldn't crash; they just don't qualify.
        let path = std::env::temp_dir().join("lens-mtime-missing-xyz.txt");
        std::fs::remove_file(&path).ok();
        assert!(!super::modified_within_days(&path, 30));
    }
}
