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
        DuplicationMode::Sonar => crate::analyzer::sonar_dup::detect_sonar_sq(files, min_lines, normalize_identifiers),
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
    /// Block size: number of statements (lines) per block for Rabin-Karp
    /// hashing. Matches SonarQube's default.
    const BLOCK_SIZE: usize = 10;
    /// Minimum number of blocks a file must contain with the same hash in
    /// order to count as a duplicate. SonarQube uses ~100 tokens ≈ 2 blocks
    /// of 10 statements each.
    let min_blocks_per_file = std::cmp::max(2, min_lines.div_ceil(BLOCK_SIZE));

    // 1. Convert tokens to per-line "statement" values (one per non-empty
    //    line). Each value is the normalized token text for that line.
    let per_file: Vec<(PathBuf, Vec<(u32, String)>)> = files
        .iter()
        .map(|(path, tokens)| {
            (
                path.clone(),
                tokens_to_statement_values(tokens, normalize_identifiers),
            )
        })
        .collect();
    // Total lines = max line number across all tokens in all files.
    // This matches SonarQube: file.getFileAttributes().getLines().
    let total_lines: u64 = files
        .iter()
        .map(|(_, tokens)| {
            tokens
                .iter()
                .map(|t| t.line)
                .max()
                .unwrap_or(0) as u64
        })
        .sum();

    // 2. Apply SonarQube's consecutive-duplicate filter (from BlockChunker).
    let per_file_filtered: Vec<(PathBuf, Vec<(u32, String)>)> = per_file
        .into_iter()
        .map(|(p, stmts)| (p, collapse_consecutive_runs(stmts)))
        .collect();

    // 3. Rabin-Karp rolling hash, one hash per sliding window of
    //    BLOCK_SIZE consecutive statements. Base 31, u64 arithmetic.
    #[derive(Clone)]
    struct BlockInfo {
        hash: u64,
        file_idx: usize,
        start_line: u32,
        end_line: u32,
    }
    let mut all_blocks: Vec<BlockInfo> = Vec::new();
    for (file_idx, (_, stmts)) in per_file_filtered.iter().enumerate() {
        if stmts.len() < BLOCK_SIZE {
            continue;
        }
        let values: Vec<&str> = stmts.iter().map(|(_, v)| v.as_str()).collect();
        let hashes = rabin_karp_blocks(&values, BLOCK_SIZE);
        for (i, h) in hashes.iter().enumerate() {
            all_blocks.push(BlockInfo {
                hash: *h,
                file_idx,
                start_line: stmts[i].0,
                end_line: stmts[i + BLOCK_SIZE - 1].0,
            });
        }
    }

    // 4. Group blocks by file. Each (file, hash) entry holds ALL the
    //    line ranges where that hash appears in that file (a file can
    //    have the same block hash at multiple non-consecutive positions;
    //    we must keep all of them, not just one — overwriting would
    //    make the result depend on HashMap iteration order, which is
    //    non-deterministic in Rust).
    //
    //    We use BTreeMap (not HashMap) so that the iteration order is
    //    deterministic across runs. HashMap uses a random seed for
    //    DoS protection, which would make file-pair processing order
    //    non-deterministic and cause different totals on each run.
    let mut by_file_blocks: BTreeMap<usize, BTreeMap<u64, Vec<(u32, u32)>>> = BTreeMap::new();
    for block in &all_blocks {
        by_file_blocks
            .entry(block.file_idx)
            .or_default()
            .entry(block.hash)
            .or_default()
            .push((block.start_line, block.end_line));
    }

    let file_indices: Vec<usize> = by_file_blocks.keys().copied().collect();
    let mut blocks: Vec<DuplicateBlock> = Vec::new();
    // SonarQube uses unique line numbers per file (HashSet) to avoid
    // counting the same line multiple times when blocks overlap.
    let mut dup_lines_per_file: HashMap<usize, HashSet<u32>> = HashMap::new();

    for i in 0..file_indices.len() {
        for j in (i + 1)..file_indices.len() {
            let file_a = file_indices[i];
            let file_b = file_indices[j];
            let map_a = &by_file_blocks[&file_a];
            let map_b = &by_file_blocks[&file_b];

            // Count shared hashes (use HashSet for O(min(|A|,|B|))).
            let common: HashSet<u64> = map_a
                .keys()
                .filter(|h| map_b.contains_key(h))
                .copied()
                .collect();
            if common.len() < min_blocks_per_file {
                continue;
            }
            // Bounding line range of ALL matching blocks in each file.
            let min_start_a = common
                .iter()
                .flat_map(|h| map_a[h].iter().map(|(s, _)| *s))
                .min()
                .unwrap();
            let max_end_a = common
                .iter()
                .flat_map(|h| map_a[h].iter().map(|(_, e)| *e))
                .max()
                .unwrap();
            let min_start_b = common
                .iter()
                .flat_map(|h| map_b[h].iter().map(|(s, _)| *s))
                .min()
                .unwrap();
            let max_end_b = common
                .iter()
                .flat_map(|h| map_b[h].iter().map(|(_, e)| *e))
                .max()
                .unwrap();

            let block_len = (max_end_a - min_start_a + 1) as usize;
            blocks.push(DuplicateBlock {
                // Semantic overload: in Sonar mode, this stores line count.
                token_count: block_len,
                occurrences: vec![
                    BlockOccurrence {
                        file: files[file_a].0.clone(),
                        start_line: min_start_a,
                        end_line: max_end_a,
                    },
                    BlockOccurrence {
                        file: files[file_b].0.clone(),
                        start_line: min_start_b,
                        end_line: max_end_b,
                    },
                ],
            });
            // Collect unique duplicated line numbers for each file.
            // This matches SonarQube's approach: HashSet<Integer> per file.
            let set_a = dup_lines_per_file.entry(file_a).or_insert_with(HashSet::new);
            for line in min_start_a..=max_end_a {
                set_a.insert(line);
            }
            let set_b = dup_lines_per_file.entry(file_b).or_insert_with(HashSet::new);
            for line in min_start_b..=max_end_b {
                set_b.insert(line);
            }
        }
    }

    // 4b. Merge blocks that refer to the same set of files. With the
    //     file-pair approach, a 3-file clone produces 3 blocks (A↔B,
    //     A↔C, B↔C). We want 1 report per clone, so we group by the
    //     sorted set of files and union the line ranges.
    for block in blocks.iter_mut() {
        block.occurrences.sort_by(|a, b| a.file.cmp(&b.file));
    }
    let mut grouped: HashMap<Vec<PathBuf>, DuplicateBlock> = HashMap::new();
    for block in blocks {
        let files_key: Vec<PathBuf> = block.occurrences.iter().map(|o| o.file.clone()).collect();
        grouped
            .entry(files_key)
            .and_modify(|existing| {
                for (i, occ) in block.occurrences.iter().enumerate() {
                    if let Some(existing_occ) = existing.occurrences.get_mut(i) {
                        existing_occ.start_line = existing_occ.start_line.min(occ.start_line);
                        existing_occ.end_line = existing_occ.end_line.max(occ.end_line);
                    }
                }
                existing.token_count = existing
                    .occurrences
                    .iter()
                    .map(|o| (o.end_line - o.start_line + 1) as usize)
                    .max()
                    .unwrap_or(0);
            })
            .or_insert(block);
    }
    let mut blocks: Vec<DuplicateBlock> = grouped.into_values().collect();

    // 5. Sort, dedupe overlapping, keep top 20.
    blocks.sort_by(|a, b| b.token_count.cmp(&a.token_count));
    let blocks = dedupe_overlapping(blocks);
    let blocks: Vec<DuplicateBlock> = blocks.into_iter().take(20).collect();

    // 6. Top offenders.
    let per_file_dup: Vec<(PathBuf, u64)> = dup_lines_per_file
        .iter()
        .map(|(idx, set)| (files[*idx].0.clone(), set.len() as u64))
        .collect();
    let duplicated_lines: u64 = dup_lines_per_file.values().map(|s| s.len() as u64).sum();
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
        total_tokens: total_lines,
        duplicated_tokens: duplicated_lines,
        duplication_percent: percent,
        min_tokens_threshold: min_lines,
        k_shingle: 0,
        winnow_window: 0,
        files_with_duplication: offenders.len(),
        top_offenders: offenders,
        shared_fingerprint_count: 0,
        blocks,
    }
}

/// Group tokens by source line, joining each line's normalized token
/// text into a single "statement" value. Empty lines (no tokens) are
/// dropped. When `normalize_identifiers` is true, identifier-like tokens
/// are replaced with `"@id"`.
fn tokens_to_statement_values(tokens: &[Token], normalize_identifiers: bool) -> Vec<(u32, String)> {
    let mut by_line: BTreeMap<u32, Vec<&Token>> = BTreeMap::new();
    for tok in tokens {
        by_line.entry(tok.line).or_default().push(tok);
    }
    let mut lines: Vec<u32> = by_line.keys().copied().collect();
    lines.sort();
    lines
        .into_iter()
        .map(|line| {
            let toks = &by_line[&line];
            let value: String = toks
                .iter()
                .map(|t| {
                    if normalize_identifiers && is_identifier(&t.text) {
                        "@id".to_string()
                    } else {
                        t.text.trim().to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            (line, value)
        })
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

/// SonarQube's `BlockChunker` consecutive-duplicate filter (translated
/// from Java):
///   - Run of 1:  keep the only item.
///   - Run of 2:  keep only the first.
///   - Run of 3+:  keep the first and the last.
fn collapse_consecutive_runs(items: Vec<(u32, String)>) -> Vec<(u32, String)> {
    let mut result = Vec::with_capacity(items.len());
    let mut i = 0;
    while i < items.len() {
        let mut j = i + 1;
        while j < items.len() && items[j].1 == items[i].1 {
            j += 1;
        }
        result.push(items[i].clone());
        if j - i >= 3 {
            result.push(items[j - 1].clone());
        }
        i = j;
    }
    result
}

/// Rabin-Karp rolling hash with base 31, one hash per sliding window of
/// `block_size` consecutive items. Matches SonarQube's `BlockChunker`
/// formula: `s[0]*31^(n-1) + s[1]*31^(n-2) + ... + s[n-1]`.
fn rabin_karp_blocks<T: Hash>(items: &[T], block_size: usize) -> Vec<u64> {
    if items.len() < block_size {
        return vec![];
    }
    let hashes: Vec<u64> = items
        .iter()
        .map(|item| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            item.hash(&mut h);
            h.finish()
        })
        .collect();

    let mut power: u64 = 1;
    for _ in 0..block_size - 1 {
        power = power.wrapping_mul(31);
    }

    // Seed: hash(items[0..block_size-1]) without the last term yet.
    let mut h: u64 = 0;
    for i in 0..block_size - 1 {
        h = h.wrapping_mul(31).wrapping_add(hashes[i]);
    }

    let mut result = Vec::with_capacity(items.len().saturating_sub(block_size - 1));
    for i in block_size - 1..items.len() {
        h = h.wrapping_mul(31).wrapping_add(hashes[i]);
        result.push(h);
        // The oldest index in the current window: i - (block_size - 1).
        // Safe because the loop starts at i == block_size - 1, so this is
        // always >= 0; using wrapping_sub to keep usize arithmetic
        // overflow-safe even in debug builds.
        let oldest_idx = i.wrapping_sub(block_size - 1);
        let oldest = hashes[oldest_idx];
        h = h.wrapping_sub(oldest.wrapping_mul(power));
    }
    result
}

/// Heuristic: a token is treated as an identifier if it starts with an
/// ASCII letter or `_` and contains only ASCII letters, digits, and `_`.
fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
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
        let min_idx = hashes.iter().enumerate().min_by_key(|(_, h)| *h).unwrap().0;
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

        if let Some((start_a, start_b, len)) = longest_common_substring(&f_a.hashes, &f_b.hashes) {
            if len >= min_fingerprints {
                // Token count is the actual span of source tokens covered
                // by the run (not the dense approximation `k + len - 1`).
                let token_a_start = f_a.token_positions[start_a];
                let token_a_end = f_a.token_positions[start_a + len - 1] + k - 1;
                let token_b_start = f_b.token_positions[start_b];
                let token_b_end = f_b.token_positions[start_b + len - 1] + k - 1;
                let token_count =
                    (token_a_end - token_a_start + 1).max(token_b_end - token_b_start + 1);

                let occ_a = BlockOccurrence {
                    file: f_a.path.clone(),
                    start_line: f_a.line_numbers.get(token_a_start).copied().unwrap_or(0),
                    end_line: f_a.line_numbers.get(token_a_end).copied().unwrap_or(0),
                };
                let occ_b = BlockOccurrence {
                    file: f_b.path.clone(),
                    start_line: f_b.line_numbers.get(token_b_start).copied().unwrap_or(0),
                    end_line: f_b.line_numbers.get(token_b_end).copied().unwrap_or(0),
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

/// Map a window position back to its source line.
#[allow(dead_code)]
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
        Token {
            text: text.to_string(),
            line: 1,
        }
    }

    #[test]
    fn identical_files_detected_as_duplicated() {
        let src = vec![
            t("function"),
            t("foo"),
            t("("),
            t(")"),
            t("{"),
            t("}"),
            t("function"),
            t("bar"),
            t("("),
            t(")"),
            t("{"),
            t("}"),
        ];
        let dup = detect(
            &[("a.ts".into(), src.clone()), ("b.ts".into(), src)],
            5,
            10,
            10,
        );
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
        Token {
            text: text.to_string(),
            line,
        }
    }

    /// Build N consecutive UNIQUE statements (each different from the
    /// others) so the consecutive-duplicate filter doesn't collapse them.
    /// Returns (path, tokens) for one file.
    fn make_unique_block(n: usize, file: &str) -> (PathBuf, Vec<Token>) {
        let mut tokens = Vec::new();
        for i in 0..n {
            let line = (i + 1) as u32;
            for word in [format!("s{}", i).as_str(), "x", ";"] {
                tokens.push(tl(word, line));
            }
        }
        (PathBuf::from(file), tokens)
    }

    #[test]
    fn sonar_identical_100_line_block_is_detected() {
        // Two files share 100 unique statements (so the consecutive-duplicate
        // filter doesn't collapse them). With block size 10, each file
        // produces 91 overlapping block hashes, all matching between the
        // two files.
        let a = make_unique_block(100, "a.ts");
        let b = make_unique_block(100, "b.ts");
        let report = detect_sonar(&[a, b], 100, false);
        assert!(
            report.duplication_percent > 0.0,
            "expected duplication > 0, got {}",
            report.duplication_percent
        );
        assert!(!report.blocks.is_empty());
        // Both files should appear in at least one block.
        let any_two = report.blocks.iter().any(|b| {
            b.occurrences
                .iter()
                .map(|o| &o.file)
                .collect::<std::collections::HashSet<_>>()
                .len()
                == 2
        });
        assert!(any_two, "expected a block to span both files");
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
            .flat_map(|line| {
                ["const", "a", "=", "1", ";"]
                    .iter()
                    .map(move |w| tl(w, line))
            })
            .collect();
        let tokens_b: Vec<Token> = (1..=100)
            .flat_map(|line| {
                ["const", "b", "=", "2", ";"]
                    .iter()
                    .map(move |w| tl(w, line))
            })
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
        let a = make_unique_block(100, "a.ts");
        let b = make_unique_block(100, "b.ts");
        let c = make_unique_block(100, "c.ts");
        let report = detect_sonar(&[a, b, c], 100, false);
        assert!(report.duplication_percent > 0.0);
        // All three files should appear in at least one block.
        // (With per-file-pair reporting, the same clone may produce 3
        // separate blocks — one for each pair — so we check the union.)
        let unique_files: std::collections::HashSet<_> = report
            .blocks
            .iter()
            .flat_map(|b| b.occurrences.iter().map(|o| &o.file))
            .collect();
        assert_eq!(
            unique_files.len(),
            3,
            "expected all three files to be covered"
        );
    }

    #[test]
    fn sonar_collapse_filter_reduces_aa_to_a() {
        // Direct unit test for the SonarQube consecutive-duplicate filter.
        // Three identical statements followed by a fourth different one.
        let items = vec![
            (1u32, "a".to_string()),
            (2u32, "a".to_string()),
            (3u32, "a".to_string()),
            (4u32, "b".to_string()),
        ];
        let collapsed = collapse_consecutive_runs(items);
        // 3+ identical: keep first and last. Then the different one.
        assert_eq!(collapsed.len(), 3);
        assert_eq!(collapsed[0].1, "a");
        assert_eq!(collapsed[1].1, "a");
        assert_eq!(collapsed[2].1, "b");
    }

    #[test]
    fn sonar_collapse_filter_handles_pairs() {
        // Two identical: keep only the first.
        let items = vec![
            (1u32, "a".to_string()),
            (2u32, "a".to_string()),
            (3u32, "b".to_string()),
        ];
        let collapsed = collapse_consecutive_runs(items);
        assert_eq!(collapsed.len(), 2);
        assert_eq!(collapsed[0].1, "a");
        assert_eq!(collapsed[1].1, "b");
    }

    #[test]
    fn detect_with_mode_dispatches_to_sonar() {
        // Two files sharing 100 unique statements so the consecutive filter
        // doesn't collapse them.
        let a = make_unique_block(100, "a.ts");
        let b = make_unique_block(100, "b.ts");
        let report = detect_with_mode(&[a, b], DuplicationMode::Sonar, 5, 10, 100, 100, false);
        assert_eq!(report.mode, DuplicationMode::Sonar);
        assert!(report.duplication_percent > 0.0);
    }

    #[test]
    fn sonar_normalized_catches_renamed_identifiers() {
        // Two files that differ ONLY by identifier names. Each line has
        // a different non-identifier constant (0, 1, …, 19) so the
        // statements are all distinct after normalization (the consecutive
        // filter doesn't collapse them).
        let make = |fname: &str, id: &str| {
            let mut tokens = Vec::new();
            for i in 0..20 {
                let line = (i + 1) as u32;
                for word in [id, format!("{}", i).as_str(), ";"] {
                    tokens.push(tl(word, line));
                }
            }
            (PathBuf::from(fname), tokens)
        };
        let a = make("a.ts", "foo");
        let b = make("b.ts", "bar");
        let files: Vec<(PathBuf, Vec<Token>)> = vec![a, b];

        // Without normalization: no match (different identifiers).
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
            "normalized mode should match (renamed identifiers), got {}",
            normalized.duplication_percent
        );
        // Both files should appear in some block.
        let unique_files: std::collections::HashSet<_> = normalized
            .blocks
            .iter()
            .flat_map(|b| b.occurrences.iter().map(|o| &o.file))
            .collect();
        assert_eq!(unique_files.len(), 2, "expected both files to be covered");
    }

    #[test]
    fn block_detection_finds_repeated_function() {
        // Build two token streams that contain a clear duplicate block of
        // ~80 tokens. With k=5, winnow window=10, and min_tokens=30 we
        // expect at least one block to be detected.
        let shared: Vec<Token> = (0..80).map(|i| t(&format!("tok{}", i))).collect();
        let mut a = shared.clone();
        a.push(t("end"));
        let mut b = shared.clone();
        b.push(t("end"));

        let dup = detect(&[("a.ts".into(), a), ("b.ts".into(), b)], 5, 10, 30);
        assert!(!dup.blocks.is_empty(), "expected at least one block, got 0");
        let block = &dup.blocks[0];
        assert!(block.token_count >= 30, "token_count={}", block.token_count);
        assert_eq!(block.occurrences.len(), 2);
    }
}
