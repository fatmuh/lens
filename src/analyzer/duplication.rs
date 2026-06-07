//! Token-level duplication detection with block-level reporting.
//!
//! Algorithm (Sonar-style):
//! 1. Tokenize each file (comments and strings stripped — see `tokenize`).
//! 2. For each file, generate k-gram shingles of tokens.
//! 3. Apply winnowing over a sliding window to pick representative hashes
//!    ("fingerprints"), keeping the source token position of each.
//! 4. Group all fingerprints by hash across all files.
//! 5. For each hash that appears in ≥ 2 files, count it toward the summary
//!    statistics (`shared_fingerprint_count`, `duplicated_tokens`).
//! 6. For the top file pairs (by shared-hash count), find the longest
//!    common substring of fingerprint arrays using DP. Map the result back
//!    to source line ranges using the stored token positions.
//!
//! This gives both the project-wide percentage and the actual line ranges
//! of the most important duplicate blocks, which is what a developer needs
//! to act on the findings.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::analyzer::tokenize::Token;

/// Which duplication algorithm to use.
///
/// * `Token` (default): our shingling + winnowing approach, sensitive to
///   small blocks (~20 lines).
/// * `Sonar`: SonarQube-compatible line-based algorithm — only flags blocks
///   of 100+ identical lines that appear in ≥ 2 files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DuplicationMode {
    #[default]
    Token,
    Sonar,
}

impl DuplicationMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "token" | "tokens" | "shingle" => Some(Self::Token),
            "sonar" | "line" | "lines" | "line-based" | "sonar-compat" => Some(Self::Sonar),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlockOccurrence {
    pub file: PathBuf,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone)]
pub struct DuplicateBlock {
    pub token_count: usize,
    pub occurrences: Vec<BlockOccurrence>,
}

#[derive(Debug, Clone)]
pub struct DuplicationReport {
    /// Which algorithm produced this report.
    pub mode: DuplicationMode,
    /// In Token mode: total tokens across all analyzed files.
    /// In Sonar mode: total source lines across all analyzed files.
    pub total_tokens: u64,
    /// In Token mode: tokens that share a fingerprint with another file.
    /// In Sonar mode: lines that are part of a duplicate block.
    pub duplicated_tokens: u64,
    pub duplication_percent: f64,
    /// In Token mode: `min_duplicate_tokens` (block size in tokens).
    /// In Sonar mode: `min_duplicate_lines` (block size in lines).
    pub min_tokens_threshold: usize,
    pub k_shingle: usize,
    pub winnow_window: usize,
    pub files_with_duplication: usize,
    /// Files with the highest count of duplicated fingerprints, up to 10.
    pub top_offenders: Vec<(PathBuf, u64)>,
    /// Number of distinct fingerprint hashes shared by ≥ 2 files.
    /// (Only populated in Token mode; 0 in Sonar mode.)
    pub shared_fingerprint_count: u64,
    /// Largest duplicate blocks across the project (up to 20).
    pub blocks: Vec<DuplicateBlock>,
}

struct FileFingerprints {
    path: PathBuf,
    hashes: Vec<u64>,
    token_positions: Vec<usize>,
    line_numbers: Vec<u32>,
}

/// Detect duplication across a set of tokenized files.
pub fn detect(
    files: &[(PathBuf, Vec<Token>)],
    k: usize,
    window: usize,
    min_tokens: usize,
) -> DuplicationReport {
    let total_tokens: u64 = files.iter().map(|(_, t)| t.len() as u64).sum();

    if total_tokens < min_tokens as u64 {
        return DuplicationReport {
            mode: DuplicationMode::Token,
            total_tokens,
            duplicated_tokens: 0,
            duplication_percent: 0.0,
            min_tokens_threshold: min_tokens,
            k_shingle: k,
            winnow_window: window,
            files_with_duplication: 0,
            top_offenders: vec![],
            shared_fingerprint_count: 0,
            blocks: vec![],
        };
    }

    // 1. Compute per-file fingerprints with token positions.
    let file_fps: Vec<FileFingerprints> = files
        .iter()
        .filter_map(|(p, toks)| {
            if toks.len() < k {
                return None;
            }
            let (hashes, token_positions) = fingerprints_with_positions(toks, k, window);
            let line_numbers: Vec<u32> = toks.iter().map(|t| t.line).collect();
            Some(FileFingerprints {
                path: p.clone(),
                hashes,
                token_positions,
                line_numbers,
            })
        })
        .collect();

    // 2. Build hash -> set of file indices.
    let mut hash_to_files: HashMap<u64, HashSet<usize>> = HashMap::new();
    for (i, f) in file_fps.iter().enumerate() {
        let mut seen: HashSet<u64> = HashSet::new();
        for &h in &f.hashes {
            if seen.insert(h) {
                hash_to_files.entry(h).or_default().insert(i);
            }
        }
    }

    // 3. Summary stats.
    let mut duplicated_tokens: u64 = 0;
    let mut per_file_duplicated: HashMap<PathBuf, u64> = HashMap::new();
    let mut shared_count: u64 = 0;
    for f in &file_fps {
        let mut local_dup: u64 = 0;
        let mut seen: HashSet<u64> = HashSet::new();
        for &h in &f.hashes {
            if !seen.insert(h) {
                continue;
            }
            if let Some(set) = hash_to_files.get(&h) {
                if set.len() >= 2 {
                    local_dup += k as u64;
                    shared_count += 1;
                }
            }
        }
        if local_dup > 0 {
            per_file_duplicated.insert(f.path.clone(), local_dup);
        }
    }
    for (_, set) in &hash_to_files {
        if set.len() >= 2 {
            duplicated_tokens += (set.len() as u64 - 1) * k as u64;
        }
    }

    let duplication_percent = if total_tokens > 0 {
        (duplicated_tokens as f64 / total_tokens as f64) * 100.0
    } else {
        0.0
    };

    // 4. Top offenders.
    let mut offenders: Vec<(PathBuf, u64)> = per_file_duplicated.into_iter().collect();
    offenders.sort_by(|a, b| b.1.cmp(&a.1));
    offenders.truncate(10);
    let files_with_duplication = offenders.len();

    // 5. Block-level detection.
    let blocks = find_blocks(&file_fps, &hash_to_files, k, window, min_tokens);

    DuplicationReport {
        mode: DuplicationMode::Token,
        total_tokens,
        duplicated_tokens,
        duplication_percent,
        min_tokens_threshold: min_tokens,
        k_shingle: k,
        winnow_window: window,
        files_with_duplication,
        top_offenders: offenders,
        shared_fingerprint_count: shared_count,
        blocks,
    }
}

/// Dispatch to the algorithm selected by `mode`.
pub fn detect_with_mode(
    files: &[(PathBuf, Vec<Token>)],
    mode: DuplicationMode,
    k: usize,
    window: usize,
    min_tokens: usize,
    min_lines: usize,
    normalize_identifiers: bool,
) -> DuplicationReport {
    match mode {
        DuplicationMode::Token => detect(files, k, window, min_tokens),
        DuplicationMode::Sonar => detect_sonar(files, min_lines, normalize_identifiers),
    }
}

/// SonarQube-compatible line-based duplication detection.
///
/// Algorithm:
///   1. For each line in each file, hash the (whitespace-normalized) tokens
///      that fall on that line.
///   2. Within each file, find maximal runs of consecutive lines that share
///      the same hash.
///   3. A run is a *candidate* duplicate block if its length ≥ `min_lines`.
///   4. Group candidates by `(hash, length)`. A group is a *real* duplicate
///      block if it appears in ≥ 2 files.
///   5. Project-wide `%` = (sum of duplicate block lengths) / total lines.
///
/// Differences from SonarQube that we know about:
///   * SonarQube uses a language-specific lexer; we use a generic
///     whitespace-stripped token hash. This means identifier-case is
///     preserved (correctly) but we may differ on string-literal handling.
///   * SonarQube has post-processing for "almost-duplicate" regions; we
///     require exact line equality.
///   * SonarQube's default `min_lines` is 100, configurable per project.
pub fn detect_sonar(
    files: &[(PathBuf, Vec<Token>)],
    min_lines: usize,
    normalize_identifiers: bool,
) -> DuplicationReport {
    // 1. Per-file, per-line hashes.
    let per_file: Vec<Vec<(u32, u64)>> = files
        .iter()
        .map(|(_, toks)| compute_line_hashes(toks, normalize_identifiers))
        .collect();
    let total_lines: u64 = per_file.iter().map(|v| v.len() as u64).sum();

    // 2. Find runs of identical consecutive lines in each file.
    let mut hash_to_runs: HashMap<u64, Vec<(usize, u32, u32)>> = HashMap::new();
    for (file_idx, line_hashes) in per_file.iter().enumerate() {
        for (hash, start, end) in find_consecutive_runs(line_hashes) {
            let len = (end - start + 1) as usize;
            if len >= min_lines {
                hash_to_runs.entry(hash).or_default().push((file_idx, start, end));
            }
        }
    }

    // 3. Group runs by (hash, length) so we can find ALL occurrences of
    //    each candidate block.
    let mut block_groups: HashMap<(u64, u32), Vec<(usize, u32, u32)>> = HashMap::new();
    for (hash, runs) in &hash_to_runs {
        for (file_idx, start, end) in runs {
            let len = end - start + 1;
            block_groups.entry((*hash, len)).or_default().push((*file_idx, *start, *end));
        }
    }

    // 4. For each (hash, length) group: if 2+ distinct files have a run,
    //    emit a duplicate block.
    let mut blocks: Vec<DuplicateBlock> = Vec::new();
    let mut duplicated_lines: u64 = 0;
    let mut per_file_dup: HashMap<PathBuf, u64> = HashMap::new();

    for (_key, occurrences) in &block_groups {
        let unique_files: HashSet<usize> = occurrences.iter().map(|(f, _, _)| *f).collect();
        if unique_files.len() < 2 {
            continue;
        }
        let block_len = occurrences[0].2 - occurrences[0].1 + 1;
        let occs: Vec<BlockOccurrence> = occurrences
            .iter()
            .map(|(f, s, e)| BlockOccurrence {
                file: files[*f].0.clone(),
                start_line: *s,
                end_line: *e,
            })
            .collect();
        blocks.push(DuplicateBlock {
            // Semantic overload: in Sonar mode, this field stores the
            // block size in lines, not tokens.
            token_count: block_len as usize,
            occurrences: occs,
        });
        // Count each block's lines once toward the project-wide %.
        duplicated_lines += block_len as u64;
        for (f, _, _) in occurrences {
            *per_file_dup.entry(files[*f].0.clone()).or_insert(0) += block_len as u64;
        }
    }

    // 5. Sort by size, dedupe overlapping, keep top 20.
    blocks.sort_by(|a, b| b.token_count.cmp(&a.token_count));
    let blocks = dedupe_overlapping(blocks);
    let blocks: Vec<DuplicateBlock> = blocks.into_iter().take(20).collect();

    // 6. Top offenders.
    let mut offenders: Vec<(PathBuf, u64)> = per_file_dup.into_iter().collect();
    offenders.sort_by(|a, b| b.1.cmp(&a.1));
    offenders.truncate(10);

    let percent = if total_lines > 0 {
        (duplicated_lines as f64 / total_lines as f64) * 100.0
    } else {
        0.0
    };

    DuplicationReport {
        mode: DuplicationMode::Sonar,
        total_tokens: total_lines,        // semantic overload: total lines
        duplicated_tokens: duplicated_lines, // semantic overload: duplicated lines
        duplication_percent: percent,
        min_tokens_threshold: min_lines,  // semantic overload: min lines
        k_shingle: 0,
        winnow_window: 0,
        files_with_duplication: offenders.len(),
        top_offenders: offenders,
        shared_fingerprint_count: 0,
        blocks,
    }
}

/// Group tokens by source line and produce a hash for each line's
/// whitespace-normalized token sequence.
///
/// When `normalize_identifiers` is true, tokens that look like identifiers
/// (a-zA-Z0-9_, starting with letter or _) are replaced with the literal
/// `"@id"` before hashing. This makes the line hash invariant to variable
/// renames — `function add(a, b) { return a + b; }` and
/// `function sum(x, y) { return x + y; }` then produce the same hash,
/// which is closer to how SonarQube's "duplications" metric behaves.
fn compute_line_hashes(
    tokens: &[Token],
    normalize_identifiers: bool,
) -> Vec<(u32, u64)> {
    let mut by_line: BTreeMap<u32, Vec<&Token>> = BTreeMap::new();
    for tok in tokens {
        by_line.entry(tok.line).or_default().push(tok);
    }
    by_line
        .iter()
        .map(|(line_num, toks)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for tok in toks {
                let normalized = if normalize_identifiers && is_identifier(&tok.text) {
                    "@id"
                } else {
                    tok.text.trim()
                };
                normalized.hash(&mut h);
            }
            (*line_num, h.finish())
        })
        .collect()
}

/// Heuristic: a token is treated as an identifier if it starts with an
/// ASCII letter or `_` and contains only ASCII letters, digits, and `_`.
/// This is intentionally permissive (it doesn't know about language-specific
/// keywords) but is good enough for line-hash normalization in practice.
fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Find maximal runs of consecutive lines with the same hash.
/// Returns `(hash, start_line, end_line)` for each run of length ≥ 1.
fn find_consecutive_runs(line_hashes: &[(u32, u64)]) -> Vec<(u64, u32, u32)> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < line_hashes.len() {
        let mut j = i + 1;
        while j < line_hashes.len() && line_hashes[j].1 == line_hashes[i].1 {
            j += 1;
        }
        // Always emit a run; the caller filters by length.
        runs.push((line_hashes[i].1, line_hashes[i].0, line_hashes[j - 1].0));
        i = j;
    }
    runs
}

/// Generate fingerprints (hash + source token position) for one file.
fn fingerprints_with_positions(
    tokens: &[Token],
    k: usize,
    window: usize,
) -> (Vec<u64>, Vec<usize>) {
    if tokens.len() < k {
        return (vec![], vec![]);
    }
    let n_shingles = tokens.len() - k + 1;
    let mut shingles: Vec<u64> = Vec::with_capacity(n_shingles);
    for i in 0..n_shingles {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for tok in &tokens[i..i + k] {
            tok.text.hash(&mut h);
        }
        shingles.push(h.finish());
    }
    let indices = winnow_indices(&shingles, window);
    let hashes: Vec<u64> = indices.iter().map(|&i| shingles[i]).collect();
    let positions: Vec<usize> = indices;
    (hashes, positions)
}

/// Winnowing that returns indices into the hash array (instead of the
/// hashes themselves), so we can keep the source token positions.
fn winnow_indices(hashes: &[u64], window: usize) -> Vec<usize> {
    if hashes.is_empty() {
        return vec![];
    }
    if hashes.len() <= window {
        let min_idx = hashes
            .iter()
            .enumerate()
            .min_by_key(|(_, h)| *h)
            .unwrap()
            .0;
        return vec![min_idx];
    }

    let mut selected: Vec<usize> = Vec::new();
    let mut last_selected_idx: Option<usize> = None;

    let mut i = 0;
    while i + window <= hashes.len() {
        let mut min_idx = i;
        let mut min_val = hashes[i];
        for j in (i + 1)..(i + window) {
            if hashes[j] <= min_val {
                min_val = hashes[j];
                min_idx = j;
            }
        }
        match last_selected_idx {
            None => {
                selected.push(min_idx);
                last_selected_idx = Some(min_idx);
            }
            Some(prev) if min_idx > prev => {
                selected.push(min_idx);
                last_selected_idx = Some(min_idx);
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(prev) = last_selected_idx {
        if prev + 1 < hashes.len() {
            let (min_offset, _) = hashes[prev + 1..]
                .iter()
                .enumerate()
                .min_by_key(|(_, v)| *v)
                .unwrap();
            selected.push(prev + 1 + min_offset);
        }
    }

    selected
}

/// Find the largest duplicate blocks by running longest-common-substring
/// DP on the fingerprint arrays of the most-similar file pairs.
fn find_blocks(
    files: &[FileFingerprints],
    hash_files: &HashMap<u64, HashSet<usize>>,
    k: usize,
    window: usize,
    min_tokens: usize,
) -> Vec<DuplicateBlock> {
    if files.len() < 2 {
        return vec![];
    }

    // 1. Count shared hashes per file pair.
    let mut pair_shared: HashMap<(usize, usize), u32> = HashMap::new();
    for (_, file_set) in hash_files {
        if file_set.len() < 2 {
            continue;
        }
        let mut vs: Vec<usize> = file_set.iter().copied().collect();
        vs.sort_unstable();
        for i in 0..vs.len() {
            for j in (i + 1)..vs.len() {
                *pair_shared.entry((vs[i], vs[j])).or_insert(0) += 1;
            }
        }
    }

    // 2. Sort by shared count, take top 500 to bound work.
    const MAX_PAIRS: usize = 500;
    let mut pairs: Vec<((usize, usize), u32)> = pair_shared.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1));
    pairs.truncate(MAX_PAIRS);

    let min_fingerprints = (min_tokens + window - 1) / window;
    let mut blocks: Vec<DuplicateBlock> = Vec::new();

    for ((i, j), _) in &pairs {
        let f_a = &files[*i];
        let f_b = &files[*j];

        // Bound DP cost: skip pairs where the product of sizes is too
        // large (O(m*n) per LCS, m*n > 4M is slow).
        if f_a.hashes.len() * f_b.hashes.len() > 4_000_000 {
            continue;
        }

        if let Some((start_a, start_b, len)) =
            longest_common_substring(&f_a.hashes, &f_b.hashes)
        {
            if len >= min_fingerprints {
                // Token count is the actual span of source tokens covered
                // by the run (not the dense approximation `k + len - 1`).
                let token_a_start = f_a.token_positions[start_a];
                let token_a_end = f_a.token_positions[start_a + len - 1] + k - 1;
                let token_b_start = f_b.token_positions[start_b];
                let token_b_end = f_b.token_positions[start_b + len - 1] + k - 1;
                let token_count = (token_a_end - token_a_start + 1)
                    .max(token_b_end - token_b_start + 1);

                let occ_a = BlockOccurrence {
                    file: f_a.path.clone(),
                    start_line: f_a
                        .line_numbers
                        .get(token_a_start)
                        .copied()
                        .unwrap_or(0),
                    end_line: f_a
                        .line_numbers
                        .get(token_a_end)
                        .copied()
                        .unwrap_or(0),
                };
                let occ_b = BlockOccurrence {
                    file: f_b.path.clone(),
                    start_line: f_b
                        .line_numbers
                        .get(token_b_start)
                        .copied()
                        .unwrap_or(0),
                    end_line: f_b
                        .line_numbers
                        .get(token_b_end)
                        .copied()
                        .unwrap_or(0),
                };
                blocks.push(DuplicateBlock {
                    token_count,
                    occurrences: vec![occ_a, occ_b],
                });
            }
        }
    }

    // 3. Sort by token count, descending, and dedupe overlapping.
    blocks.sort_by(|a, b| b.token_count.cmp(&a.token_count));
    let mut blocks = dedupe_overlapping(blocks);

    // 4. Keep top 20.
    blocks.truncate(20);
    blocks
}

/// Find the longest common substring of two arrays of u64.
/// Returns (start_a, start_b, length). Memory: O(n) using rolling DP.
fn longest_common_substring(a: &[u64], b: &[u64]) -> Option<(usize, usize, usize)> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let m = a.len();
    let n = b.len();

    let mut prev: Vec<u32> = vec![0; n + 1];
    let mut curr: Vec<u32> = vec![0; n + 1];

    let mut best_len: u32 = 0;
    let mut best_a_end = 0usize;
    let mut best_b_end = 0usize;

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                curr[j] = prev[j - 1] + 1;
                if curr[j] > best_len {
                    best_len = curr[j];
                    best_a_end = i;
                    best_b_end = j;
                }
            } else {
                curr[j] = 0;
            }
        }
        std::mem::swap(&mut prev, &mut curr);
        for x in curr.iter_mut() {
            *x = 0;
        }
    }

    if best_len == 0 {
        None
    } else {
        let len = best_len as usize;
        Some((best_a_end - len, best_b_end - len, len))
    }
}

fn token_to_line(f: &FileFingerprints, win_pos: usize, _k: usize) -> u32 {
    let token_pos = f.token_positions.get(win_pos).copied().unwrap_or(0);
    f.line_numbers.get(token_pos).copied().unwrap_or(0)
}

/// Drop blocks that are subsumed by a larger block in the same file.
fn dedupe_overlapping(blocks: Vec<DuplicateBlock>) -> Vec<DuplicateBlock> {
    // Simple heuristic: a block is "dominated" if another block in the
    // report covers the same file at a wider range. We sort by token count
    // descending and drop dominated ones.
    let mut sorted = blocks;
    sorted.sort_by(|a, b| b.token_count.cmp(&a.token_count));

    let mut kept: Vec<DuplicateBlock> = Vec::new();
    for cand in sorted {
        let dominated = kept.iter().any(|k| {
            k.occurrences.iter().any(|ko| {
                cand.occurrences.iter().any(|co| {
                    ko.file == co.file
                        && ko.start_line <= co.start_line
                        && ko.end_line >= co.end_line
                        && (ko.end_line - ko.start_line) > (co.end_line - co.start_line)
                })
            })
        });
        if !dominated {
            kept.push(cand);
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(text: &str) -> Token {
        Token { text: text.to_string(), line: 1 }
    }

    #[test]
    fn identical_files_detected_as_duplicated() {
        let src = vec![
            t("function"), t("foo"), t("("), t(")"), t("{"), t("}"),
            t("function"), t("bar"), t("("), t(")"), t("{"), t("}"),
        ];
        let dup = detect(&[("a.ts".into(), src.clone()), ("b.ts".into(), src)], 5, 10, 10);
        assert!(dup.duplication_percent > 0.0);
        assert_eq!(dup.files_with_duplication, 2);
    }

    #[test]
    fn unique_files_have_no_duplication() {
        let a: Vec<Token> = (0..20).map(|i| t(&format!("a{}", i))).collect();
        let b: Vec<Token> = (0..20).map(|i| t(&format!("b{}", i))).collect();
        let dup = detect(&[("a.ts".into(), a), ("b.ts".into(), b)], 5, 10, 10);
        assert_eq!(dup.duplicated_tokens, 0);
    }

    #[test]
    fn winnow_indices_matches_winnow() {
        let hashes = vec![5, 3, 8, 1, 9, 2, 7, 4, 6, 0];
        let indices = winnow_indices(&hashes, 3);
        // Each selected index must correspond to a hash that's the rightmost
        // minimum in some window.
        assert!(!indices.is_empty());
        // Indices must be strictly increasing.
        for w in indices.windows(2) {
            assert!(w[0] < w[1], "indices not increasing: {:?}", indices);
        }
    }

    #[test]
    fn longest_common_substring_finds_match() {
        let a: Vec<u64> = (1..=10).collect();
        let b: Vec<u64> = vec![100, 200, 3, 4, 5, 6, 7, 300];
        let (sa, sb, len) = longest_common_substring(&a, &b).unwrap();
        assert_eq!(len, 5);
        assert_eq!(sa, 2);
        assert_eq!(sb, 2);
    }

    #[test]
    fn longest_common_substring_no_match() {
        let a: Vec<u64> = vec![1, 2, 3];
        let b: Vec<u64> = vec![4, 5, 6];
        assert!(longest_common_substring(&a, &b).is_none());
    }

    // --- SonarQube-compatible (line-based) mode tests ---

    /// Build a `Token` with a specific line number so we can test line-based
    /// grouping.
    fn tl(text: &str, line: u32) -> Token {
        Token { text: text.to_string(), line }
    }

    #[test]
    fn sonar_identical_100_line_block_is_detected() {
        // Build two files that share a 100-line block of identical content.
        let mut tokens_a = Vec::new();
        let mut tokens_b = Vec::new();
        for line in 1..=100u32 {
            for word in ["const", "x", "=", "1", ";"] {
                tokens_a.push(tl(word, line));
                tokens_b.push(tl(word, line));
            }
        }
        let report = detect_sonar(
            &[("a.ts".into(), tokens_a), ("b.ts".into(), tokens_b)],
            100,
            false,
        );
        assert!(
            report.duplication_percent > 0.0,
            "expected duplication > 0, got {}",
            report.duplication_percent
        );
        assert_eq!(report.blocks.len(), 1);
        assert_eq!(report.blocks[0].token_count, 100);
        assert_eq!(report.blocks[0].occurrences.len(), 2);
    }

    #[test]
    fn sonar_below_threshold_is_ignored() {
        // Two files share a 50-line block. With min_lines=100, this is ignored.
        let mut tokens_a = Vec::new();
        let mut tokens_b = Vec::new();
        for line in 1..=50u32 {
            for word in ["const", "x", "=", "1", ";"] {
                tokens_a.push(tl(word, line));
                tokens_b.push(tl(word, line));
            }
        }
        let report = detect_sonar(
            &[("a.ts".into(), tokens_a), ("b.ts".into(), tokens_b)],
            100,
            false,
        );
        assert_eq!(report.duplication_percent, 0.0);
        assert!(report.blocks.is_empty());
    }

    #[test]
    fn sonar_intra_file_duplicate_is_ignored() {
        // A single file with a duplicate block — we only count cross-file.
        let mut tokens = Vec::new();
        for _ in 0..2 {
            for line in 1..=100u32 {
                for word in ["const", "x", "=", "1", ";"] {
                    tokens.push(tl(word, line));
                }
            }
            // Bump to a different line range for the second occurrence.
            for line in 200..=299u32 {
                for word in ["const", "x", "=", "1", ";"] {
                    tokens.push(tl(word, line));
                }
            }
        }
        let report = detect_sonar(&[("a.ts".into(), tokens)], 100, false);
        assert_eq!(report.duplication_percent, 0.0);
        assert!(report.blocks.is_empty());
    }

    #[test]
    fn sonar_different_content_not_matched() {
        // Two files with different content per line.
        let tokens_a: Vec<Token> = (1..=100)
            .flat_map(|line| ["const", "a", "=", "1", ";"].iter().map(move |w| tl(w, line)))
            .collect();
        let tokens_b: Vec<Token> = (1..=100)
            .flat_map(|line| ["const", "b", "=", "2", ";"].iter().map(move |w| tl(w, line)))
            .collect();
        let report = detect_sonar(
            &[("a.ts".into(), tokens_a), ("b.ts".into(), tokens_b)],
            50,
            false,
        );
        assert_eq!(report.duplication_percent, 0.0);
        assert!(report.blocks.is_empty());
    }

    #[test]
    fn sonar_three_files_with_same_block() {
        let mut tokens_a = Vec::new();
        let mut tokens_b = Vec::new();
        let mut tokens_c = Vec::new();
        for line in 1..=100u32 {
            for word in ["let", "v", "=", "42", ";"] {
                tokens_a.push(tl(word, line));
                tokens_b.push(tl(word, line));
                tokens_c.push(tl(word, line));
            }
        }
        let report = detect_sonar(
            &[
                ("a.ts".into(), tokens_a),
                ("b.ts".into(), tokens_b),
                ("c.ts".into(), tokens_c),
            ],
            100,
            false,
        );
        assert!(report.duplication_percent > 0.0);
        // All three files should be in the occurrences list.
        assert_eq!(report.blocks[0].occurrences.len(), 3);
    }

    #[test]
    fn detect_with_mode_dispatches_to_sonar() {
        // Two files sharing a 100-line identical block.
        let mut tokens_a = Vec::new();
        let mut tokens_b = Vec::new();
        for line in 1..=100u32 {
            for word in ["const", "x", "=", "1", ";"] {
                tokens_a.push(tl(word, line));
                tokens_b.push(tl(word, line));
            }
        }
        let report = detect_with_mode(
            &[("a.ts".into(), tokens_a), ("b.ts".into(), tokens_b)],
            DuplicationMode::Sonar,
            5,
            10,
            100,
            100,
            false,
        );
        assert_eq!(report.mode, DuplicationMode::Sonar);
        assert!(report.duplication_percent > 0.0);
    }

    #[test]
    fn sonar_normalized_catches_renamed_identifiers() {
        // Two blocks that differ ONLY by identifier names.
        // a.ts:   function add(a, b) { return a + b; }
        // b.ts:   function sum(x, y) { return x + y; }
        // With normalization, these should match. Without, they should not.
        let make = |fname: &str, fnname: &str, p1: &str, p2: &str| {
            let mut tokens = Vec::new();
            let mut line = 1u32;
            for word in ["function", fnname, "(", p1, ",", p2, ")", "{"] {
                tokens.push(tl(word, line));
            }
            line += 1;
            for word in ["return", p1, "+", p2, ";", "}"] {
                tokens.push(tl(word, line));
            }
            (PathBuf::from(fname), tokens)
        };
        let a = make("a.ts", "add", "a", "b");
        let b = make("b.ts", "sum", "x", "y");
        let files: Vec<(PathBuf, Vec<Token>)> = vec![a, b];

        // Without normalization: no match.
        let strict = detect_sonar(&files, 1, false);
        assert_eq!(
            strict.duplication_percent, 0.0,
            "strict mode should NOT match (different identifiers)"
        );
        assert!(strict.blocks.is_empty());

        // With normalization: match (identifiers are replaced with @id).
        let normalized = detect_sonar(&files, 1, true);
        assert!(
            normalized.duplication_percent > 0.0,
            "normalized mode should match (renamed identifiers)"
        );
        // Two duplicate blocks: the `function … {` line and the `return …;` line.
        assert!(
            !normalized.blocks.is_empty(),
            "expected at least one block, got {}",
            normalized.blocks.len()
        );
    }

    #[test]
    fn block_detection_finds_repeated_function() {
        // Build two token streams that contain a clear duplicate block of
        // ~80 tokens. With k=5, winnow window=10, and min_tokens=30 we
        // expect at least one block to be detected.
        let shared: Vec<Token> = (0..80)
            .map(|i| t(&format!("tok{}", i)))
            .collect();
        let mut a = shared.clone();
        a.push(t("end"));
        let mut b = shared.clone();
        b.push(t("end"));

        let dup = detect(
            &[("a.ts".into(), a), ("b.ts".into(), b)],
            5,
            10,
            30,
        );
        assert!(!dup.blocks.is_empty(), "expected at least one block, got 0");
        let block = &dup.blocks[0];
        assert!(
            block.token_count >= 30,
            "token_count={}",
            block.token_count
        );
        assert_eq!(block.occurrences.len(), 2);
    }
}
