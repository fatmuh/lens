//! SonarQube's OriginalCloneDetectionAlgorithm ported to Rust.
//!
//! Based on the paper "Index-Based Code Clone Detection: Incremental,
//! Distributed, Scalable" by Hummel, Juergens, Conradt & Heinemann.
//!
//! Algorithm:
//!   1. Tokenize files → group tokens per line (TokensLine)
//!   2. Collapse consecutive duplicate lines (SonarQube's BlockChunker filter)
//!   3. Rabin-Karp rolling hash over sliding windows of BLOCK_SIZE lines
//!   4. Build CloneIndex: HashMap<hash, Vec<Block>> across all files
//!   5. For each file, run active-set intersection to find maximal clones
//!   6. Filter: remove clones fully contained in larger clones
//!   7. Calculate % using unique line numbers per file (HashSet)

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::analyzer::tokenize::Token;
use crate::analyzer::duplication::{BlockOccurrence, DuplicateBlock, DuplicationMode, DuplicationReport};

/// Block size for Rabin-Karp hashing.
/// SQ default=10 statements, but our "statements" are coarser (per-line),
/// so we use a smaller value to compensate.
const BLOCK_SIZE: usize = 5;
/// Rabin-Karp prime base.
const PRIME_BASE: u64 = 31;

// ── Data structures ──────────────────────────────────────────────────

/// A block produced by Rabin-Karp chunking. Analogous to SonarQube's `Block`.
#[derive(Debug, Clone)]
struct SqBlock {
    /// File path (resource_id in SQ).
    resource_id: PathBuf,
    /// Position of this block in the file's block list (indexInFile).
    index_in_file: usize,
    /// First source line of this block.
    start_line: u32,
    /// Last source line of this block.
    end_line: u32,
    /// Rabin-Karp hash of BLOCK_SIZE consecutive TokensLines.
    hash: u64,
}

/// A clone group found by the algorithm. Analogous to SonarQube's `CloneGroup`.
#[derive(Debug, Clone)]
struct SqCloneGroup {
    /// Length in blocks (number of consecutive blocks).
    length: usize,
    /// The parts (occurrences) of this clone.
    parts: Vec<SqClonePart>,
}

#[derive(Debug, Clone)]
struct SqClonePart {
    resource_id: PathBuf,
    index_in_file: usize,
    start_line: u32,
    end_line: u32,
}

/// A group of blocks sorted by (resource_id, index_in_file).
/// Analogous to SonarQube's `BlocksGroup`.
#[derive(Clone)]
struct BlocksGroup {
    blocks: Vec<SqBlock>,
}

impl BlocksGroup {
    fn empty() -> Self {
        BlocksGroup { blocks: vec![] }
    }

    fn size(&self) -> usize {
        self.blocks.len()
    }

    /// Find first block with given resource_id.
    fn first(&self, resource_id: &PathBuf) -> Option<&SqBlock> {
        self.blocks.iter().find(|b| b.resource_id == *resource_id)
    }

    /// Intersection: blocks from `other` that have a corresponding block
    /// in `self` with same resource_id and index_in_file = self.index + 1.
    /// This is the core of the active-set shrinking.
    fn intersect(&self, other: &BlocksGroup) -> BlocksGroup {
        let mut result = BlocksGroup::empty();
        let list1 = &self.blocks;
        let list2 = &other.blocks;
        let mut i = 0;
        let mut j = 0;
        while i < list1.len() && j < list2.len() {
            let b1 = &list1[i];
            let b2 = &list2[j];
            let cmp_res = compare_blocks(b1, b2);
            if cmp_res == 0 {
                // Same resource_id → check index correction (+1)
                let idx_cmp = (b1.index_in_file as i64 + 1) - b2.index_in_file as i64;
                if idx_cmp == 0 {
                    result.blocks.push(b2.clone());
                    i += 1;
                    j += 1;
                } else if idx_cmp > 0 {
                    j += 1;
                } else {
                    i += 1;
                }
            } else if cmp_res < 0 {
                i += 1;
            } else {
                j += 1;
            }
        }
        result
    }

    /// Check if this group is subsumed by `other` with index correction.
    /// "Subsumed" means every block in self has a corresponding block in other
    /// with same resource_id and index = other.index + index_correction.
    fn subsumed_by(&self, other: &BlocksGroup, index_correction: usize) -> bool {
        let list1 = &self.blocks;
        let list2 = &other.blocks;
        let mut i = 0;
        let mut j = 0;
        while i < list1.len() && j < list2.len() {
            let b1 = &list1[i];
            let b2 = &list2[j];
            let cmp_res = compare_blocks_resource_id(b1, b2);
            if cmp_res != 0 {
                j += 1;
                continue;
            }
            let idx_cmp =
                b1.index_in_file as i64 - index_correction as i64 - b2.index_in_file as i64;
            if idx_cmp < 0 {
                break;
            }
            if idx_cmp != 0 {
                j += 1;
            }
            if idx_cmp == 0 {
                i += 1;
                j += 1;
            }
        }
        i == list1.len()
    }

    /// Match blocks from begin group with blocks from end group.
    /// Pairs up blocks with same resource_id where begin.index + len - 1 == end.index.
    fn pairs(&self, end_group: &BlocksGroup, len: usize) -> Vec<(SqBlock, SqBlock)> {
        let mut result = vec![];
        let begins = &self.blocks;
        let ends = &end_group.blocks;
        let mut i = 0;
        let mut j = 0;
        while i < begins.len() && j < ends.len() {
            let bb = &begins[i];
            let eb = &ends[j];
            let cmp_res = compare_blocks_resource_id(bb, eb);
            if cmp_res == 0 {
                let idx_cmp = bb.index_in_file as i64 + len as i64 - 1 - eb.index_in_file as i64;
                if idx_cmp == 0 {
                    result.push((bb.clone(), eb.clone()));
                    i += 1;
                    j += 1;
                } else if idx_cmp > 0 {
                    j += 1;
                } else {
                    i += 1;
                }
            } else if cmp_res > 0 {
                j += 1;
            } else {
                i += 1;
            }
        }
        result
    }
}

/// Compare blocks by resource_id (path string comparison).
fn compare_blocks_resource_id(a: &SqBlock, b: &SqBlock) -> i64 {
    let s1 = a.resource_id.to_string_lossy();
    let s2 = b.resource_id.to_string_lossy();
    match s1.cmp(&s2) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// Compare blocks by (resource_id, index_in_file).
fn compare_blocks(a: &SqBlock, b: &SqBlock) -> i64 {
    let rid = compare_blocks_resource_id(a, b);
    if rid != 0 {
        return rid;
    }
    a.index_in_file as i64 - b.index_in_file as i64
}

/// Sort blocks by (resource_id, index_in_file).
fn sort_blocks(blocks: &mut Vec<SqBlock>) {
    blocks.sort_by(|a, b| {
        match a.resource_id.to_string_lossy().cmp(&b.resource_id.to_string_lossy()) {
            std::cmp::Ordering::Equal => a.index_in_file.cmp(&b.index_in_file),
            other => other,
        }
    });
}

// ── CloneIndex ───────────────────────────────────────────────────────

/// Index of all blocks across all files, keyed by hash.
struct CloneIndex {
    by_hash: HashMap<u64, Vec<SqBlock>>,
}

impl CloneIndex {
    fn new() -> Self {
        CloneIndex {
            by_hash: HashMap::new(),
        }
    }

    fn add(&mut self, block: SqBlock) {
        self.by_hash.entry(block.hash).or_default().push(block);
    }

    fn get(&self, hash: u64) -> &[SqBlock] {
        self.by_hash.get(&hash).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

// ── Containment Filter ───────────────────────────────────────────────

/// SonarQube's Filter: removes clones fully contained in larger clones.
fn filter_contained(groups: Vec<SqCloneGroup>) -> Vec<SqCloneGroup> {
    let mut filtered: Vec<SqCloneGroup> = Vec::new();
    for current in groups {
        let mut dominated = false;
        let mut to_remove: Vec<usize> = vec![];
        for (idx, earlier) in filtered.iter().enumerate() {
            if contains_in(&current, earlier) {
                // current is contained in earlier → skip
                dominated = true;
                break;
            }
            if contains_in(earlier, &current) {
                // earlier is contained in current → remove earlier
                to_remove.push(idx);
            }
        }
        if !dominated {
            // Remove dominated earlier groups (reverse order)
            for &idx in to_remove.iter().rev() {
                filtered.remove(idx);
            }
            filtered.push(current);
        }
    }
    filtered
}

/// Check if `first` clone is contained in `second`.
/// A clone A is contained in B if every part of A has a corresponding part
/// in B with same resource_id and B covers A's range.
fn contains_in(first: &SqCloneGroup, second: &SqCloneGroup) -> bool {
    if first.length > second.length {
        return false;
    }
    // Check all parts of first are covered by parts of second
    for pa in &first.parts {
        let mut found = false;
        for pb in &second.parts {
            if pa.resource_id == pb.resource_id
                && pb.start_line <= pa.start_line
                && pa.end_line <= pb.end_line
            {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    // Also check all resource_ids of second exist in first (symmetric check)
    for pb in &second.parts {
        let mut found = false;
        for pa in &first.parts {
            if pa.resource_id == pb.resource_id {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

// ── Main detection ───────────────────────────────────────────────────

/// Run the SonarQube-compatible duplication detection.
/// Port of SonarQube's OriginalCloneDetectionAlgorithm.
pub fn detect_sonar_sq(
    files: &[(PathBuf, Vec<Token>)],
    _min_lines: usize,
    normalize_identifiers: bool,
) -> DuplicationReport {
    // ── Step 1: Group tokens per line (TokensLine) ──
    let per_file: Vec<(PathBuf, Vec<(u32, String)>)> = files
        .iter()
        .map(|(path, tokens)| {
            (
                path.clone(),
                tokens_to_statement_values(tokens, normalize_identifiers),
            )
        })
        .collect();

    // Total lines = max line number across all tokens (SonarQube: file.getLines())
    let total_lines: u64 = files
        .iter()
        .map(|(_, tokens)| tokens.iter().map(|t| t.line).max().unwrap_or(0) as u64)
        .sum();

    // ── Step 2: Collapse consecutive duplicate lines ──
    let per_file_filtered: Vec<(PathBuf, Vec<(u32, String)>)> = per_file
        .into_iter()
        .map(|(p, stmts)| (p, collapse_consecutive_runs(stmts)))
        .collect();

    // ── Step 3: Rabin-Karp chunking → Blocks ──
    let mut index = CloneIndex::new();
    // Keep per-file block lists (sorted by index_in_file) for the detection loop.
    let mut file_blocks: Vec<(PathBuf, Vec<SqBlock>)> = Vec::new();

    for (file_idx, (path, stmts)) in per_file_filtered.iter().enumerate() {
        if stmts.len() < BLOCK_SIZE {
//             // eprintln!("SKIP {} : {} stmts (need {})", path.display(), stmts.len(), BLOCK_SIZE);
            file_blocks.push((path.clone(), vec![]));
            continue;
        }
        let values: Vec<&str> = stmts.iter().map(|(_, v)| v.as_str()).collect();
        let hashes = rabin_karp_blocks(&values, BLOCK_SIZE);

        let mut blocks_for_file: Vec<SqBlock> = Vec::new();
        for (i, &hash) in hashes.iter().enumerate() {
            let block = SqBlock {
                resource_id: path.clone(),
                index_in_file: i,
                start_line: stmts[i].0,
                end_line: stmts[i + BLOCK_SIZE - 1].0,
                hash,
            };
            index.add(block.clone());
            blocks_for_file.push(block);
        }
        file_blocks.push((path.clone(), blocks_for_file));
    }

    let files_with_blocks: Vec<_> = file_blocks.iter().filter(|(_, b)| !b.is_empty()).collect();
//     eprintln!("Files with blocks: {}/{}", files_with_blocks.len(), file_blocks.len());

    // ── Step 4: Sort index entries by (resource_id, index_in_file) ──
    for blocks in index.by_hash.values_mut() {
        sort_blocks(blocks);
    }

    // ── Step 5: Run OriginalCloneDetectionAlgorithm per file ──
    let mut all_groups: Vec<SqCloneGroup> = Vec::new();

    for (origin_path, blocks) in &file_blocks {
        if blocks.is_empty() {
            continue;
        }
        let groups = find_clones_for_file(&index, origin_path, blocks);
        all_groups.extend(groups);
    }

    // ── Step 5.5: Filter by minimum clone length ──
    // SonarQube: NumberOfUnitsNotLessThan(minimumTokens=100)
    // Each block = ~10 statements. minimumTokens=100 => min 10 blocks.
//     eprintln!("Before min_block filter: {} groups", all_groups.len());
    let min_clone_blocks = 1;
    all_groups.retain(|g| g.length >= min_clone_blocks);
//     eprintln!("After min_block filter: {} groups", all_groups.len());

    // ── Step 6: Filter contained clones ──
    // Sort by length descending first (so larger clones are processed first)
    all_groups.sort_by(|a, b| b.length.cmp(&a.length));
    // Debug
//     // eprintln!("Before filter: {} groups", all_groups.len());
    let filtered = filter_contained(all_groups);
//     // eprintln!("After filter: {} groups", filtered.len());

    // ── Step 7: Build report ──
    // Calculate unique duplicated line numbers per file.
    let mut dup_lines_per_file: HashMap<PathBuf, HashSet<u32>> = HashMap::new();

    let mut report_blocks: Vec<DuplicateBlock> = Vec::new();
    for group in &filtered {
        let mut occurrences: Vec<BlockOccurrence> = Vec::new();
        for part in &group.parts {
            for line in part.start_line..=part.end_line {
                dup_lines_per_file
                    .entry(part.resource_id.clone())
                    .or_default()
                    .insert(line);
            }
            occurrences.push(BlockOccurrence {
                file: part.resource_id.clone(),
                start_line: part.start_line,
                end_line: part.end_line,
            });
        }
        let line_count = occurrences
            .iter()
            .map(|o| (o.end_line - o.start_line + 1) as usize)
            .max()
            .unwrap_or(0);
        report_blocks.push(DuplicateBlock {
            token_count: line_count,
            occurrences,
        });
    }

    // Sort blocks by size, take top 20
    report_blocks.sort_by(|a, b| b.token_count.cmp(&a.token_count));
    report_blocks.truncate(20);

    // Top offenders
    let mut offenders: Vec<(PathBuf, u64)> = dup_lines_per_file
        .iter()
        .map(|(p, set)| (p.clone(), set.len() as u64))
        .collect();
    offenders.sort_by(|a, b| b.1.cmp(&a.1));
    offenders.truncate(10);

    let duplicated_lines: u64 = dup_lines_per_file.values().map(|s| s.len() as u64).sum();
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
        min_tokens_threshold: BLOCK_SIZE,
        k_shingle: 0,
        winnow_window: 0,
        files_with_duplication: offenders.len(),
        top_offenders: offenders,
        shared_fingerprint_count: 0,
        blocks: report_blocks,
    }
}

// ── Clone detection per file ─────────────────────────────────────────

/// Port of OriginalCloneDetectionAlgorithm.findClones().
fn find_clones_for_file(
    index: &CloneIndex,
    origin_resource_id: &PathBuf,
    file_blocks: &[SqBlock],
) -> Vec<SqCloneGroup> {
    let size = file_blocks.len();

    // Create groups: one BlocksGroup per unique hash, containing all blocks
    // from ALL files that share that hash.
    let mut groups_by_hash: HashMap<u64, BlocksGroup> = HashMap::new();

    // Add current file's blocks
    for fb in file_blocks {
        groups_by_hash
            .entry(fb.hash)
            .or_insert_with(BlocksGroup::empty)
            .blocks
            .push(fb.clone());
    }

    // Add blocks from index (other files only)
    for (hash, group) in groups_by_hash.iter_mut() {
        for idx_block in index.get(*hash) {
            if idx_block.resource_id != *origin_resource_id {
                group.blocks.push(idx_block.clone());
            }
        }
        sort_blocks(&mut group.blocks);
    }

    // Build sameHashBlocksGroups array: c[0]=empty, c[1..size]=groups, c[size+1]=empty
    let mut c: Vec<Option<BlocksGroup>> = vec![None; size + 2];
    c[0] = Some(BlocksGroup::empty());
    for fb in file_blocks {
        let i = fb.index_in_file + 1;
        c[i] = Some(
            groups_by_hash
                .get(&fb.hash)
                .cloned()
                .unwrap_or_else(BlocksGroup::empty),
        );
    }
    c[size + 1] = Some(BlocksGroup::empty());

    let mut results: Vec<SqCloneGroup> = Vec::new();
    let mut outer_count = 0usize;
    let mut inner_hits = 0usize;

    // Outer loop
    for i in 1..=size {
        let ci = c[i].as_ref().unwrap();

        // Skip if < 2 blocks or subsumed by previous
        if ci.size() < 2 {
            continue;
        }
        outer_count += 1;
        let c_prev = c[i - 1].as_ref().unwrap();
        if ci.subsumed_by(c_prev, 1) {
            continue;
        }

        // Active set
        let mut active = ci.clone();
        let ci_clone = ci.clone(); // keep for pairs()

        // Inner loop
        for j in (i + 1)..=size + 1 {
            let cj = c[j].as_ref().unwrap();
            let a0 = active.intersect(cj);

            if a0.size() < active.size() {
                // Clone ends — report if origin block matches
                if let Some(origin_block) = active.first(origin_resource_id) {
                    if origin_block.index_in_file == j - 2 {
                        let length = j - i;
                        let pairs = ci_clone.pairs(&active, length);
                        if !pairs.is_empty() {
                            let parts: Vec<SqClonePart> = pairs
                                .into_iter()
                                .map(|(begin, end)| SqClonePart {
                                    resource_id: begin.resource_id,
                                    index_in_file: begin.index_in_file,
                                    start_line: begin.start_line,
                                    end_line: end.end_line,
                                })
                                .collect();
                            results.push(SqCloneGroup { length, parts });
                            inner_hits += 1;
                        }
                    }
                }
            }

            active = a0;

            if active.size() < 2 {
                break;
            }
            let c_prev2 = c[i - 1].as_ref().unwrap();
            if active.subsumed_by(c_prev2, j - i + 1) {
                break;
            }
        }
    }

//     eprintln!("  {} blocks, outer={}, hits={}, results={}", size, outer_count, inner_hits, results.len());
    results
}

// ── Helper functions (reused from original) ──────────────────────────

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
        power = power.wrapping_mul(PRIME_BASE);
    }

    let mut h: u64 = 0;
    for i in 0..block_size - 1 {
        h = h.wrapping_mul(PRIME_BASE).wrapping_add(hashes[i]);
    }

    let mut result = Vec::with_capacity(items.len().saturating_sub(block_size - 1));
    for i in block_size - 1..items.len() {
        h = h.wrapping_mul(PRIME_BASE).wrapping_add(hashes[i]);
        result.push(h);
        let oldest_idx = i.wrapping_sub(block_size - 1);
        let oldest = hashes[oldest_idx];
        h = h.wrapping_sub(oldest.wrapping_mul(power));
    }
    result
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
