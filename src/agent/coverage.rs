//! Coverage agent — fills coverage gaps by generating tests for uncovered lines.
//!
//! Reads the LCOV report, finds uncovered lines, and asks the AI to write
//! minimal tests that cover those lines **without changing any existing code**.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::client::{chat, AiConfig};
use super::diff::PendingChange;

/// Result of a coverage fix run.
#[derive(Debug)]
pub struct CoverageFixResult {
    pub test_files_written: Vec<PathBuf>,
    pub lines_covered: u32,
    /// Pending changes collected (used for dry-run preview).
    pub pending: Vec<PendingChange>,
}

/// Find uncovered lines from an LCOV file and generate tests.
///
/// `project_root` — the project being fixed
/// `lcov_path` — path to the LCOV coverage report
/// `max_files` — maximum number of source files to process per run
pub async fn fix_uncovered(
    config: &AiConfig,
    project_root: &Path,
    lcov_path: &Path,
    max_files: usize,
) -> Result<CoverageFixResult> {
    let uncovered = parse_uncovered_lines(lcov_path)?;
    if uncovered.is_empty() {
        println!("  {} All lines already covered!", "✓".green());
        return Ok(CoverageFixResult {
            test_files_written: vec![],
            lines_covered: 0,
            pending: vec![],
        });
    }

    println!(
        "  {} Found {} files with uncovered lines",
        "🔍".to_string().cyan(),
        uncovered.len(),
    );

    let mut result = CoverageFixResult {
        test_files_written: vec![],
        lines_covered: 0,
        pending: vec![],
    };

    for (file, lines) in uncovered.iter().take(max_files) {
        let source_path = project_root.join(file);
        if !source_path.exists() {
            continue;
        }

        let source_code = std::fs::read_to_string(&source_path).context("reading source file")?;

        let line_numbers: Vec<u32> = lines.iter().copied().collect();
        println!(
            "  {} Generating tests for {} ({} uncovered lines: {:?})",
            "📝".to_string().yellow(),
            file,
            lines.len(),
            &line_numbers[..line_numbers.len().min(10)],
        );

        let system_prompt = build_coverage_system_prompt();
        let user_prompt = format!(
            "Source file: {}\n\nUncovered lines: {:?}\n\nSource code:\n```typescript\n{}\n```\n\n\
             Write a test file that covers the uncovered lines. \
             Import the module/functions from the source file. \
             Use Jest as the test framework. \
             Only output the test file content, nothing else. \
             Do NOT modify the source file.",
            file, line_numbers, source_code,
        );

        match chat(config, &system_prompt, &user_prompt).await {
            Ok(test_code) => {
                // Extract code from markdown code block if present
                let test_code = extract_code_block(&test_code);
                let test_path = derive_test_path(project_root, file);
                let change = PendingChange::new(&test_path, test_code);
                println!(
                    "  {} Prepared {} ({} bytes)",
                    "✓".green(),
                    test_path.display(),
                    change.new_content().len(),
                );
                result.test_files_written.push(test_path);
                result.lines_covered += lines.len() as u32;
                result.pending.push(change);
            }
            Err(e) => {
                eprintln!("  {} AI error for {}: {}", "✗".red(), file, e);
            }
        }
    }

    Ok(result)
}

/// Parse uncovered lines from an LCOV file.
/// Returns a map of file → sorted list of uncovered line numbers.
fn parse_uncovered_lines(path: &Path) -> Result<std::collections::BTreeMap<String, Vec<u32>>> {
    let content = std::fs::read_to_string(path).context("reading LCOV file")?;
    let mut result: std::collections::BTreeMap<String, Vec<u32>> =
        std::collections::BTreeMap::new();
    let mut current_file = String::new();

    for line in content.lines() {
        if let Some(f) = line.strip_prefix("SF:") {
            current_file = f.to_string();
        } else if let Some(da) = line.strip_prefix("DA:") {
            // DA:line_number,hit_count
            let parts: Vec<&str> = da.split(',').collect();
            if parts.len() >= 2 {
                if let (Ok(ln), Ok(hits)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                    if hits == 0 {
                        result.entry(current_file.clone()).or_default().push(ln);
                    }
                }
            }
        }
    }

    Ok(result)
}

fn build_coverage_system_prompt() -> String {
    "You are a test-writing agent. Your job is to write minimal, focused tests \
     that cover specific uncovered lines in TypeScript/JavaScript code.\n\n\
     Rules:\n\
     1. Only write tests — NEVER modify the source file\n\
     2. Use Jest as the test framework (describe/test/expect)\n\
     3. Import from the source file using relative paths\n\
     4. Cover ONLY the uncovered lines mentioned\n\
     5. Keep tests minimal — no unnecessary assertions\n\
     6. Do not change the behavior of the existing application\n\
     7. Output only the test file content, no explanations\n\
     8. Use TypeScript (.ts) for the test file"
        .to_string()
}

fn extract_code_block(s: &str) -> String {
    // If wrapped in ```typescript ... ``` or ``` ... ```, extract the content.
    if let Some(start) = s.find("```") {
        let after_start = &s[start + 3..];
        // Skip optional language tag on first line
        let content_start = after_start.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &after_start[content_start..];
        if let Some(end) = content.find("```") {
            return content[..end].trim().to_string();
        }
    }
    s.trim().to_string()
}

fn derive_test_path(root: &Path, source: &str) -> PathBuf {
    // src/foo/bar.ts → tests/foo/bar.test.ts
    let rel = source.strip_prefix("src/").unwrap_or(source);
    let stem = rel
        .strip_suffix(".ts")
        .or_else(|| rel.strip_suffix(".tsx"))
        .unwrap_or(rel);
    root.join("tests").join(format!("{}.test.ts", stem))
}

// --- owo_colors trait for colored output in this module ---
use owo_colors::OwoColorize;
