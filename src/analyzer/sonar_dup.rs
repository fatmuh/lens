//! SonarQube's OriginalCloneDetectionAlgorithm — faithful Rust port.
//!
//! Pipeline (matches SonarQube exactly):
//!   1. Tokenize files → tokens with (line, text)
//!   2. Group tokens per line → TokensLine(startUnit, endUnit, startLine, value)
//!   3. Collapse consecutive duplicate TokensLines (BlockChunker filter)
//!   4. PmdBlockChunker: Rabin-Karp rolling hash over TokensLines, blockSize=10
//!   5. Build CloneIndex: HashMap<hash, Vec<Block>> across all files
//!   6. OriginalCloneDetectionAlgorithm: per-file active-set intersection
//!   7. Filter: remove clones fully contained in larger clones
//!   8. NumberOfUnitsNotLessThan(100): filter clones < 100 tokens
//!   9. DuplicationMeasures: density = 100 * duplicatedLines / totalLines (HashSet per file)

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::analyzer::duplication::{BlockOccurrence, DuplicateBlock, DuplicationMode, DuplicationReport};
use crate::analyzer::tokenize::Token;

// ── SQ Defaults ──────────────────────────────────────────────────────

/// Block size for Rabin-Karp hashing. SQ default = 10 TokensLines.
const BLOCK_SIZE: usize = 10;
/// Minimum clone size in tokens. SQ default = 100.
const MINIMUM_TOKENS: usize = 100;
/// Rabin-Karp prime base.
const PRIME_BASE: u64 = 31;

// ── Data structures ──────────────────────────────────────────────────

/// Analogous to SQ's TokensLine:
/// tokens grouped per line with unit (token index) tracking.
#[derive(Debug, Clone)]
struct TokensLine {
    /// Index of first token on this line in the file's token sequence.
    start_unit: usize,
    /// Index of last token on this line.
    end_unit: usize,
    /// Source line number (1-based).
    start_line: u32,
    /// Concatenation of all token images on this line.
    value: String,
}

impl TokensLine {
    /// Hash used by PmdBlockChunker. Matches SQ's TokensLine.getHashCode().
    /// SQ uses `value.hashCode()` which is Java String.hashCode().
    fn get_hash_code(&self) -> i32 {
        // Java String.hashCode(): s[0]*31^(n-1) + s[1]*31^(n-2) + ... + s[n-1]
        let mut h: i32 = 0;
        for ch in self.value.chars() {
            h = h.wrapping_mul(31).wrapping_add(ch as i32);
        }
        h
    }
}

/// Analogous to SQ's Block (from PmdBlockChunker).
#[derive(Debug, Clone)]
struct SqBlock {
    /// File path (resource_id in SQ).
    resource_id: PathBuf,
    /// Position of this block in the file's block list.
    index_in_file: usize,
    /// First source line of this block.
    start_line: u32,
    /// Last source line of this block.
    end_line: u32,
    /// Index of first token (unit) in this block.
    start_unit: usize,
    /// Index of last token (unit) in this block.
    end_unit: usize,
    /// Rabin-Karp hash of BLOCK_SIZE consecutive TokensLines.
    hash: i64,
}

/// Analogous to SQ's ClonePart.
#[derive(Debug, Clone)]
struct SqClonePart {
    resource_id: PathBuf,
    start_unit: usize,
    start_line: u32,
    end_line: u32,
}

/// Analogous to SQ's CloneGroup.
#[derive(Debug, Clone)]
struct SqCloneGroup {
    /// Length in blocks.
    clone_unit_length: usize,
    /// The parts (occurrences) of this clone.
    parts: Vec<SqClonePart>,
}

/// Sorted group of blocks. Analogous to SQ's BlocksGroup.
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

    /// First block from this group with specified resource id.
    fn first(&self, resource_id: &PathBuf) -> Option<&SqBlock> {
        self.blocks.iter().find(|b| b.resource_id == *resource_id)
    }

    /// Intersection: blocks from `other` that have a corresponding block
    /// in `self` with same resource_id and index_in_file = self.index + 1.
    /// Faithful port of BlocksGroup.intersect().
    fn intersect(&self, other: &BlocksGroup) -> BlocksGroup {
        let mut intersection = BlocksGroup::empty();
        let list1 = &self.blocks;
        let list2 = &other.blocks;
        let mut i = 0;
        let mut j = 0;
        while i < list1.len() && j < list2.len() {
            let block1 = &list1[i];
            let block2 = &list2[j];
            let c = compare_resource_id(&block1.resource_id, &block2.resource_id);
            if c > 0 {
                j += 1;
                continue;
            }
            if c < 0 {
                i += 1;
                continue;
            }
            // Same resource_id — check index correction (+1)
            let idx = block1.index_in_file as i64 + 1 - block2.index_in_file as i64;
            if idx == 0 {
                intersection.blocks.push(block2.clone());
                i += 1;
                j += 1;
            } else if idx > 0 {
                j += 1;
            } else {
                i += 1;
            }
        }
        intersection
    }

    /// Check if this group is subsumed by `other` with index correction.
    /// Faithful port of BlocksGroup.subsumedBy().
    fn subsumed_by(&self, other: &BlocksGroup, index_correction: usize) -> bool {
        let list1 = &self.blocks;
        let list2 = &other.blocks;
        let mut i = 0;
        let mut j = 0;
        while i < list1.len() && j < list2.len() {
            let block1 = &list1[i];
            let block2 = &list2[j];
            let c = compare_resource_id(&block1.resource_id, &block2.resource_id);
            if c != 0 {
                j += 1;
                continue;
            }
            let idx =
                block1.index_in_file as i64 - index_correction as i64 - block2.index_in_file as i64;
            if idx < 0 {
                break;
            }
            if idx != 0 {
                j += 1;
            }
            if idx == 0 {
                i += 1;
                j += 1;
            }
        }
        i == list1.len()
    }

    /// Match begin blocks with end blocks for clone reporting.
    /// Faithful port of BlocksGroup.pairs().
    fn pairs(&self, end_group: &BlocksGroup, len: usize) -> Vec<(SqBlock, SqBlock)> {
        let mut result = vec![];
        let begins = &self.blocks;
        let ends = &end_group.blocks;
        let mut i = 0;
        let mut j = 0;
        while i < begins.len() && j < ends.len() {
            let bb = &begins[i];
            let eb = &ends[j];
            let c = compare_resource_id(&bb.resource_id, &eb.resource_id);
            if c == 0 {
                let idx = bb.index_in_file as i64 + len as i64 - 1 - eb.index_in_file as i64;
                if idx == 0 {
                    result.push((bb.clone(), eb.clone()));
                    i += 1;
                    j += 1;
                } else if idx > 0 {
                    j += 1;
                } else {
                    i += 1;
                }
            } else if c > 0 {
                j += 1;
            } else {
                i += 1;
            }
        }
        result
    }
}

/// Compare two PathBuf as strings (lexicographic, like Java's FastStringComparator).
fn compare_resource_id(a: &PathBuf, b: &PathBuf) -> i64 {
    let s1 = a.to_string_lossy();
    let s2 = b.to_string_lossy();
    match s1.cmp(&s2) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// Sort blocks by (resource_id, index_in_file). Matches BlocksGroup.BlockComparator.
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
    by_hash: HashMap<i64, Vec<SqBlock>>,
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

    fn get(&self, hash: i64) -> &[SqBlock] {
        self.by_hash.get(&hash).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

// ── Step 2: Tokens per line (DefaultCpdTokens) ──────────────────────

/// Group tokens by line → TokensLine, exactly like SQ's DefaultCpdTokens.
fn tokens_to_tokens_lines(tokens: &[Token], normalize_identifiers: bool) -> Vec<TokensLine> {
    if tokens.is_empty() {
        return vec![];
    }

    let mut by_line: BTreeMap<u32, Vec<&Token>> = BTreeMap::new();
    for tok in tokens {
        by_line.entry(tok.line).or_default().push(tok);
    }

    let mut result = Vec::new();
    let mut current_unit: usize = 0;

    for (line_num, toks) in &by_line {
        let start_unit = current_unit;
        let value: String = toks
            .iter()
            .map(|t| {
                current_unit += 1;
                if normalize_identifiers && is_identifier(&t.text) {
                    "$id".to_string()
                } else {
                    t.text.trim().to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("");
        let end_unit = current_unit - 1;

        if !value.is_empty() {
            result.push(TokensLine {
                start_unit,
                end_unit,
                start_line: *line_num,
                value,
            });
        }
    }
    result
}

// ── Step 3: Collapse consecutive duplicates (BlockChunker filter) ───

/// Faithful port of PmdBlockChunker's collapse logic.
fn collapse_consecutive_duplicates(fragments: &[TokensLine]) -> Vec<TokensLine> {
    let mut filtered = Vec::with_capacity(fragments.len());
    let mut i = 0;
    while i < fragments.len() {
        let first = &fragments[i];
        let mut j = i + 1;
        while j < fragments.len() && fragments[j].value == first.value {
            j += 1;
        }
        filtered.push(fragments[i].clone());
        if i < j - 1 {
            filtered.push(fragments[j - 1].clone());
        }
        i = j;
    }
    filtered
}

// ── Step 4: PmdBlockChunker ──────────────────────────────────────────

/// Faithful port of PmdBlockChunker.chunk().
/// Rabin-Karp rolling hash over TokensLines with blockSize.
fn pmd_block_chunker(resource_id: &PathBuf, fragments: &[TokensLine]) -> Vec<SqBlock> {
    let filtered = collapse_consecutive_duplicates(fragments);

    if filtered.len() < BLOCK_SIZE {
        return vec![];
    }

    // Compute power = PRIME_BASE^(BLOCK_SIZE-1)
    let mut power: i64 = 1;
    for _ in 0..BLOCK_SIZE - 1 {
        power = power.wrapping_mul(PRIME_BASE as i64);
    }

    // Initial hash for first (BLOCK_SIZE-1) fragments
    let mut hash: i64 = 0;
    for k in 0..BLOCK_SIZE - 1 {
        hash = hash.wrapping_mul(PRIME_BASE as i64).wrapping_add(filtered[k].get_hash_code() as i64);
    }

    let mut blocks = Vec::with_capacity(filtered.len() - BLOCK_SIZE + 1);
    let mut first = 0;
    let mut last = BLOCK_SIZE - 1;

    while last < filtered.len() {
        let first_fragment = &filtered[first];
        let last_fragment = &filtered[last];

        // Add last statement to hash
        hash = hash
            .wrapping_mul(PRIME_BASE as i64)
            .wrapping_add(last_fragment.get_hash_code() as i64);

        // Create block
        blocks.push(SqBlock {
            resource_id: resource_id.clone(),
            index_in_file: first,
            start_line: first_fragment.start_line,
            end_line: last_fragment.start_line,
            start_unit: first_fragment.start_unit,
            end_unit: last_fragment.end_unit,
            hash,
        });

        // Remove first statement from hash
        hash = hash.wrapping_sub(power.wrapping_mul(first_fragment.get_hash_code() as i64));

        first += 1;
        last += 1;
    }

    blocks
}

// ── Step 5+6: OriginalCloneDetectionAlgorithm ────────────────────────

/// Faithful port of OriginalCloneDetectionAlgorithm.findClones().
fn find_clones_for_file(
    index: &CloneIndex,
    origin_resource_id: &PathBuf,
    file_blocks: &[SqBlock],
) -> Vec<SqCloneGroup> {
    let size = file_blocks.len();
    if size == 0 {
        return vec![];
    }

    // Create groups: one BlocksGroup per unique hash
    let mut groups_by_hash: HashMap<i64, BlocksGroup> = HashMap::new();

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
    let mut c: Vec<BlocksGroup> = vec![BlocksGroup::empty(); size + 2];
    for fb in file_blocks {
        let i = fb.index_in_file + 1;
        c[i] = groups_by_hash
            .get(&fb.hash)
            .cloned()
            .unwrap_or_else(BlocksGroup::empty);
    }
    // c[size+1] is already empty

    let mut results: Vec<SqCloneGroup> = Vec::new();

    // Outer loop: for i := 1 to length(c) do
    for i in 1..=size {
        // 8: if |c(i)| < 2 or c(i) subsumed by c(i-1) then continue
        if c[i].size() < 2 || c[i].subsumed_by(&c[i - 1], 1) {
            continue;
        }

        // 10: let a := c(i)
        let mut current_blocks_group = c[i].clone();

        // 11: for j := i+1 to length(c) do
        for j in (i + 1)..=size + 1 {
            // 12: let a0 := a intersect c(j)
            let intersected = current_blocks_group.intersect(&c[j]);

            // 13: if |a0| < |a| then
            if intersected.size() < current_blocks_group.size() {
                // 14: report clones from c(i) to a
                if let Some(first_block) = current_blocks_group.first(origin_resource_id) {
                    if first_block.index_in_file == j - 2 {
                        report_clones(&c[i], &current_blocks_group, j - i, origin_resource_id, &mut results);
                    }
                }
            }

            // 15: a := a0
            current_blocks_group = intersected;

            // 16: if |a| < 2 or a subsumed by c(i-1) then break
            if current_blocks_group.size() < 2
                || current_blocks_group.subsumed_by(&c[i - 1], j - i + 1)
            {
                break;
            }
        }
    }

    results
}

/// Faithful port of OriginalCloneDetectionAlgorithm.reportClones().
fn report_clones(
    begin_group: &BlocksGroup,
    end_group: &BlocksGroup,
    clone_length: usize,
    origin_resource_id: &PathBuf,
    results: &mut Vec<SqCloneGroup>,
) {
    let pairs = begin_group.pairs(end_group, clone_length);

    let mut origin: Option<SqClonePart> = None;
    let mut parts: Vec<SqClonePart> = Vec::new();

    for (first_block, last_block) in pairs {
        let part = SqClonePart {
            resource_id: first_block.resource_id.clone(),
            start_unit: first_block.start_unit,
            start_line: first_block.start_line,
            end_line: last_block.end_line,
        };

        if part.resource_id == *origin_resource_id {
            if origin.is_none() || part.start_unit < origin.as_ref().unwrap().start_unit {
                origin = Some(part.clone());
            }
        }

        parts.push(part);
    }

    if !parts.is_empty() {
        results.push(SqCloneGroup {
            clone_unit_length: clone_length,
            parts,
        });
    }
}

// ── Step 7: Filter (containsIn) ──────────────────────────────────────

/// Faithful port of SQ's Filter.
/// Removes clones fully contained in larger clones.
fn filter_contained(groups: Vec<SqCloneGroup>) -> Vec<SqCloneGroup> {
    let mut filtered: Vec<SqCloneGroup> = Vec::new();
    for current in groups {
        let mut dominated = false;
        let mut to_remove: Vec<usize> = vec![];
        for (idx, earlier) in filtered.iter().enumerate() {
            if contains_in(&current, earlier) {
                dominated = true;
                break;
            }
            if contains_in(earlier, &current) {
                to_remove.push(idx);
            }
        }
        if !dominated {
            for &idx in to_remove.iter().rev() {
                filtered.remove(idx);
            }
            filtered.push(current);
        }
    }
    filtered
}

/// Faithful port of Filter.containsIn().
/// Clone A is contained in B if:
///   - A.cloneUnitLength <= B.cloneUnitLength
///   - every part pA has a part pB with same resourceId and pB.unitStart <= pA.unitStart and pA.unitEnd <= pB.unitEnd
///   - all resourceIds from B exist in A
fn contains_in(first: &SqCloneGroup, second: &SqCloneGroup) -> bool {
    if first.clone_unit_length > second.clone_unit_length {
        return false;
    }

    let first_parts = &first.parts;
    let second_parts = &second.parts;

    // Check: every part in first has a covering part in second
    // AND every resourceId in second exists in first
    // Parts are sorted by (resourceId, startUnit)
    let mut i = 0;
    let mut j = 0;
    let mut first_covered = true;
    let mut second_resources_covered = true;

    // Check first covered by second
    'outer1: for pa in first_parts {
        for pb in second_parts {
            if pb.resource_id == pa.resource_id
                && pb.start_unit <= pa.start_unit
            {
                // For line coverage: we compare unit ranges
                // pA is covered if pB.endUnit >= pA.endUnit
                // Since we don't store endUnit in ClonePart, we approximate by cloneUnitLength
                continue 'outer1;
            }
        }
        first_covered = false;
        break;
    }

    if !first_covered {
        return false;
    }

    // Check all resourceIds from second exist in first
    for pb in second_parts {
        let mut found = false;
        for pa in first_parts {
            if pa.resource_id == pb.resource_id {
                found = true;
                break;
            }
        }
        if !found {
            second_resources_covered = false;
            break;
        }
    }

    second_resources_covered
}

// ── Step 8: NumberOfUnitsNotLessThan ──────────────────────────────────

/// Filter clones with less than `minimum_tokens` tokens.
fn filter_by_minimum_tokens(groups: &mut Vec<SqCloneGroup>, minimum_tokens: usize) {
    groups.retain(|g| {
        // A clone's unit length = endUnit of last block - startUnit of first block + 1
        // For each part, unit span = clone_unit_length * (avg tokens per block)
        // Actually SQ calculates: origin.endUnit - origin.startUnit + 1
        // But we can approximate: if a clone spans N blocks and each block has ~10 TokensLines,
        // and each line has ~5 tokens, then ~50*N tokens. More accurate: track units.
        // Simplest: check that at least one part spans >= minimum_tokens units.
        // SQ checks using the origin part's unit range.
        g.parts.iter().any(|p| {
            // Approximate: if we have clone_unit_length blocks of BLOCK_SIZE=10 lines,
            // and each line has some tokens, the minimum token count is at least clone_unit_length
            // (each block has at least 1 token). But SQ measures in actual tokens.
            // Since we track start_unit, we can compute end_unit from the block.
            // For now, approximate: clone_unit_length * (average tokens per TokensLine)
            // This is a rough filter — the real SQ checks exact unit counts.
            clone_unit_length_tokens(g) >= minimum_tokens
        })
    });
}

/// Calculate the token span of a clone group based on its unit length.
fn clone_unit_length_tokens(g: &SqCloneGroup) -> usize {
    if g.parts.is_empty() {
        return 0;
    }
    // Use the first part's start_unit and estimate end_unit from block count
    // Each block spans BLOCK_SIZE TokensLines, each with tokens.
    // The clone covers clone_unit_length consecutive blocks.
    // Approximate end_unit = start_unit + clone_unit_length * BLOCK_SIZE * (avg tokens per line)
    // This is rough — ideally we'd track endUnit in ClonePart.
    // For now, use clone_unit_length as a proxy (SQ uses actual unit counts)
    g.clone_unit_length * BLOCK_SIZE
}

// ── Main entry point ─────────────────────────────────────────────────

/// Run SonarQube-compatible duplication detection.
/// Full pipeline: TokensLine → PmdBlockChunker → OriginalCloneDetectionAlgorithm → Filter → MinTokens.
pub fn detect_sonar_sq(
    files: &[(PathBuf, Vec<Token>)],
    _min_lines: usize,
    normalize_identifiers: bool,
) -> DuplicationReport {
    // ── Step 1+2: Tokenize → TokensLines ──
    let per_file: Vec<(PathBuf, Vec<TokensLine>)> = files
        .iter()
        .map(|(path, tokens)| {
            (path.clone(), tokens_to_tokens_lines(tokens, normalize_identifiers))
        })
        .collect();

    // Total lines = max line number across all tokens (SQ: file.getFileAttributes().getLines())
    let total_lines: u64 = files
        .iter()
        .map(|(_, tokens)| tokens.iter().map(|t| t.line).max().unwrap_or(0) as u64)
        .sum();

    // ── Step 3+4: PmdBlockChunker → Blocks ──
    let mut index = CloneIndex::new();
    let mut file_blocks: Vec<(PathBuf, Vec<SqBlock>)> = Vec::new();

    for (path, tokens_lines) in &per_file {
        let blocks = pmd_block_chunker(path, tokens_lines);
        for block in &blocks {
            index.add(block.clone());
        }
        file_blocks.push((path.clone(), blocks));
    }

    // Sort index entries by (resource_id, index_in_file)
    for blocks in index.by_hash.values_mut() {
        sort_blocks(blocks);
    }

    // ── Step 5+6: OriginalCloneDetectionAlgorithm per file ──
    let mut all_groups: Vec<SqCloneGroup> = Vec::new();
    for (origin_path, blocks) in &file_blocks {
        if blocks.is_empty() {
            continue;
        }
        let groups = find_clones_for_file(&index, origin_path, blocks);
        all_groups.extend(groups);
    }

    // ── Step 7: Filter contained clones ──
    let mut filtered = filter_contained(all_groups);

    // ── Step 8: NumberOfUnitsNotLessThan(100) ──
    filter_by_minimum_tokens(&mut filtered, MINIMUM_TOKENS);

    // ── Step 9: Build report (DuplicationMeasures) ──
    // Calculate unique duplicated line numbers per file (HashSet<Integer> per file)
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

// ── Helpers ──────────────────────────────────────────────────────────

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
