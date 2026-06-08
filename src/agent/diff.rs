//! Diff preview and dry-run support for the AI fix agent.
//!
//! Generates unified-diff style output so users can review changes
//! before files are written. When `--dry-run` is active, no files
//! are touched — only the diff is shown.

use std::path::Path;

use owo_colors::OwoColorize;

/// A pending file change: either creating a new file or modifying an existing one.
#[derive(Debug)]
pub enum PendingChange {
    /// New file that doesn't exist yet.
    Create {
        path: std::path::PathBuf,
        content: String,
    },
    /// Existing file being modified.
    Modify {
        path: std::path::PathBuf,
        old: String,
        new: String,
    },
}

impl PendingChange {
    /// Build a `PendingChange` by reading the current file content (if any).
    pub fn new(path: &Path, new_content: String) -> Self {
        let path = path.to_path_buf();
        match std::fs::read_to_string(&path) {
            Ok(old) => PendingChange::Modify {
                path,
                old,
                new: new_content,
            },
            Err(_) => PendingChange::Create {
                path,
                content: new_content,
            },
        }
    }

    /// The file path.
    pub fn path(&self) -> &Path {
        match self {
            PendingChange::Create { path, .. } | PendingChange::Modify { path, .. } => path,
        }
    }

    /// The new content that would be written.
    pub fn new_content(&self) -> &str {
        match self {
            PendingChange::Create { content, .. } => content,
            PendingChange::Modify { new, .. } => new,
        }
    }

    /// Write the file to disk.
    pub fn apply(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(self.path(), self.new_content())
    }
}

/// Print a colourised diff preview for a list of pending changes.
pub fn print_preview(changes: &[PendingChange]) {
    for change in changes {
        match change {
            PendingChange::Create { path, content } => {
                println!(
                    "\n  {} {} (new file, {} bytes)",
                    "+".green().bold(),
                    path.display().to_string().green(),
                    content.len(),
                );
                // Show first 20 lines with + prefix
                for line in content.lines().take(20) {
                    println!("  {}", format!("+{}", line).green());
                }
                let total = content.lines().count();
                if total > 20 {
                    println!("  {} ... {} more lines", "+".green(), total - 20);
                }
            }
            PendingChange::Modify { path, old, new } => {
                println!(
                    "\n  {} {}",
                    "~".yellow().bold(),
                    path.display().to_string().yellow(),
                );
                print_unified_diff(old, new);
            }
        }
    }
}

/// Print a minimal unified-diff between old and new content.
fn print_unified_diff(old: &str, new: &str) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut oi = 0usize;
    let mut ni = 0usize;
    let mut shown_lines = 0usize;
    let max_diff_lines = 50; // cap output

    // Simple patience-free diff: LCS via DP for small files, otherwise line-by-line
    let ops = compute_diff_ops(&old_lines, &new_lines);

    // Group ops into hunks
    for op in &ops {
        if shown_lines >= max_diff_lines {
            println!(
                "  {} (diff truncated, {} more changes)",
                "...".dimmed(),
                ops.len() - shown_lines,
            );
            break;
        }
        match op {
            DiffOp::Equal(line) => {
                println!("  {}", line.dimmed());
                shown_lines += 1;
            }
            DiffOp::Delete(line) => {
                println!("  {}", format!("-{}", line).red());
                shown_lines += 1;
            }
            DiffOp::Insert(line) => {
                println!("  {}", format!("+{}", line).green());
                shown_lines += 1;
            }
        }
    }
}

#[derive(Debug)]
enum DiffOp<'a> {
    Equal(&'a str),
    Delete(&'a str),
    Insert(&'a str),
}

/// Compute a simple diff using LCS (Longest Common Subsequence).
/// For files up to ~5000 lines this is fast enough.
fn compute_diff_ops<'a>(old: &'a [&str], new: &'a [&str]) -> Vec<DiffOp<'a>> {
    let m = old.len();
    let n = new.len();

    // For very large files, fall back to line-by-line comparison
    if m > 3000 || n > 3000 {
        return simple_diff(old, new);
    }

    // DP table for LCS
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if old[i - 1] == new[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to get diff ops
    let mut ops = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if old[i - 1] == new[j - 1] {
            ops.push(DiffOp::Equal(old[i - 1]));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            ops.push(DiffOp::Delete(old[i - 1]));
            i -= 1;
        } else {
            ops.push(DiffOp::Insert(new[j - 1]));
            j -= 1;
        }
    }
    while i > 0 {
        ops.push(DiffOp::Delete(old[i - 1]));
        i -= 1;
    }
    while j > 0 {
        ops.push(DiffOp::Insert(new[j - 1]));
        j -= 1;
    }
    ops.reverse();

    // Compress: collapse runs of equal lines to max 3 context lines around changes
    compress_context(&ops)
}

/// Simple O(n*m) diff for large files — skips LCS, just matches line by line
fn simple_diff<'a>(old: &'a [&str], new: &'a [&str]) -> Vec<DiffOp<'a>> {
    let mut ops = Vec::new();
    let mut oi = 0usize;
    let mut ni = 0usize;

    while oi < old.len() && ni < new.len() {
        if old[oi] == new[ni] {
            ops.push(DiffOp::Equal(old[oi]));
            oi += 1;
            ni += 1;
        } else {
            // Look ahead in new for a match
            let mut found = None;
            for (k, &line) in new.iter().enumerate().skip(ni).take(20) {
                if line == old[oi] {
                    found = Some(k);
                    break;
                }
            }
            if let Some(k) = found {
                for line in &new[ni..k] {
                    ops.push(DiffOp::Insert(line));
                }
                ni = k;
            } else {
                ops.push(DiffOp::Delete(old[oi]));
                oi += 1;
            }
        }
    }
    while oi < old.len() {
        ops.push(DiffOp::Delete(old[oi]));
        oi += 1;
    }
    while ni < new.len() {
        ops.push(DiffOp::Insert(new[ni]));
        ni += 1;
    }
    compress_context(&ops)
}

/// Keep at most 3 equal lines before/after each change to reduce noise.
fn compress_context<'a>(ops: &[DiffOp<'a>]) -> Vec<DiffOp<'a>> {
    let mut result = Vec::new();
    let mut equal_buf: Vec<&str> = Vec::new();
    let max_ctx = 3;

    for op in ops {
        match op {
            DiffOp::Equal(line) => {
                equal_buf.push(line);
            }
            DiffOp::Delete(_) | DiffOp::Insert(_) => {
                // Flush buffered equals — keep last max_ctx
                if equal_buf.len() > max_ctx {
                    if !result.is_empty() {
                        result.push(DiffOp::Equal("..."));
                    }
                    for &l in equal_buf.iter().skip(equal_buf.len() - max_ctx) {
                        result.push(DiffOp::Equal(l));
                    }
                } else {
                    for &l in &equal_buf {
                        result.push(DiffOp::Equal(l));
                    }
                }
                equal_buf.clear();
                result.push(match op {
                    DiffOp::Delete(l) => DiffOp::Delete(l),
                    DiffOp::Insert(l) => DiffOp::Insert(l),
                    DiffOp::Equal(_) => unreachable!(),
                });
            }
        }
    }
    // Flush trailing equals
    if equal_buf.len() > max_ctx {
        result.push(DiffOp::Equal("..."));
        for &l in equal_buf.iter().skip(equal_buf.len() - max_ctx) {
            result.push(DiffOp::Equal(l));
        }
    } else {
        for &l in &equal_buf {
            result.push(DiffOp::Equal(l));
        }
    }

    result
}

/// Collect pending changes, print preview, and optionally apply.
/// Returns the list of changes that were applied (empty if dry-run).
pub fn apply_or_preview(changes: Vec<PendingChange>, dry_run: bool) -> Vec<PendingChange> {
    if changes.is_empty() {
        println!("  {} No changes to apply.", "✓".green());
        return vec![];
    }

    println!(
        "\n  {} {} file(s) to {}:",
        "📋".to_string().cyan(),
        changes.len(),
        if dry_run {
            "preview (dry-run)"
        } else {
            "apply"
        },
    );

    print_preview(&changes);

    if dry_run {
        println!(
            "\n  {} Dry-run — no files were written. Remove --dry-run to apply.",
            "ℹ".to_string().blue(),
        );
        return vec![];
    }

    let mut applied = Vec::new();
    for change in &changes {
        match change.apply() {
            Ok(()) => {
                println!(
                    "  {} Written {} ({} bytes)",
                    "✓".green(),
                    change.path().display(),
                    change.new_content().len(),
                );
                applied.push(change.clone());
            }
            Err(e) => {
                eprintln!(
                    "  {} Failed to write {}: {}",
                    "✗".red(),
                    change.path().display(),
                    e,
                );
            }
        }
    }
    applied
}

// Need Clone for apply_or_preview return
impl Clone for PendingChange {
    fn clone(&self) -> Self {
        match self {
            PendingChange::Create { path, content } => PendingChange::Create {
                path: path.clone(),
                content: content.clone(),
            },
            PendingChange::Modify { path, old, new } => PendingChange::Modify {
                path: path.clone(),
                old: old.clone(),
                new: new.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_change_new_file() {
        let change = PendingChange::new(
            Path::new("/tmp/nonexistent_test_file_12345.ts"),
            "hello".into(),
        );
        assert!(matches!(change, PendingChange::Create { .. }));
    }

    #[test]
    fn test_diff_ops_equal() {
        let ops = compute_diff_ops(&["a", "b", "c"], &["a", "b", "c"]);
        assert!(ops.iter().all(|op| matches!(op, DiffOp::Equal(_))));
    }

    #[test]
    fn test_diff_ops_insert() {
        let ops = compute_diff_ops(&["a", "c"], &["a", "b", "c"]);
        let inserts: Vec<_> = ops
            .iter()
            .filter(|op| matches!(op, DiffOp::Insert(_)))
            .collect();
        assert_eq!(inserts.len(), 1);
    }

    #[test]
    fn test_diff_ops_delete() {
        let ops = compute_diff_ops(&["a", "b", "c"], &["a", "c"]);
        let deletes: Vec<_> = ops
            .iter()
            .filter(|op| matches!(op, DiffOp::Delete(_)))
            .collect();
        assert_eq!(deletes.len(), 1);
    }

    #[test]
    fn test_compress_context() {
        let ops: Vec<DiffOp> = (0..20)
            .map(|i| DiffOp::Equal(format!("line {}", i).leak() as &str))
            .chain(std::iter::once(DiffOp::Insert("new")))
            .collect();
        let compressed = compress_context(&ops);
        // Should have "..." + 3 context + insert
        assert!(compressed.len() < ops.len());
    }
}
