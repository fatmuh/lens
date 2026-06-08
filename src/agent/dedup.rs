//! Dedup agent — refactors duplicated code into shared functions.
//!
//! Reads the duplication report, finds duplicate blocks, and asks the AI
//! to extract them into a shared utility **without changing behavior**.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::client::{chat, AiConfig};

/// Result of a dedup fix run.
#[derive(Debug)]
pub struct DedupFixResult {
    pub files_modified: Vec<PathBuf>,
    pub shared_files_created: Vec<PathBuf>,
    pub blocks_refactored: u32,
}

/// Find duplicated blocks and refactor them.
///
/// `project_root` — the project being fixed
/// `min_lines` — minimum duplicate block size to consider
/// `max_blocks` — maximum number of duplicate pairs to process per run
pub async fn fix_duplicates(
    config: &AiConfig,
    project_root: &Path,
    min_lines: usize,
    max_blocks: usize,
) -> Result<DedupFixResult> {
    // Run a scan to get the duplication report.
    let scan_output = run_duplication_scan(project_root)?;
    let dup_blocks = parse_duplication_report(&scan_output, min_lines)?;

    if dup_blocks.is_empty() {
        println!("  {} No significant duplicates found!", "✓".green());
        return Ok(DedupFixResult {
            files_modified: vec![],
            shared_files_created: vec![],
            blocks_refactored: 0,
        });
    }

    println!(
        "  {} Found {} duplicate groups ({} unique blocks)",
        "🔍".to_string().cyan(),
        dup_blocks.len(),
        dup_blocks.iter().map(|b| b.locations.len()).sum::<usize>(),
    );

    let mut result = DedupFixResult {
        files_modified: vec![],
        shared_files_created: vec![],
        blocks_refactored: 0,
    };

    for block in dup_blocks.iter().take(max_blocks) {
        println!(
            "  {} Refactoring duplicate in {} locations ({} lines each)",
            "📝".to_string().yellow(),
            block.locations.len(),
            block.line_count,
        );

        // Read the first occurrence as the "original".
        let first = &block.locations[0];
        let first_path = project_root.join(&first.file);
        let source = std::fs::read_to_string(&first_path).context("reading source file")?;

        let system_prompt = build_dedup_system_prompt();
        let user_prompt = format!(
            "I have duplicated code that appears in {} locations:\n\n\
             First occurrence: {} (lines {}-{})\n\
             Duplicate code snippet:\n```\n{}\n```\n\n\
             Other locations:\n{}\n\n\
             Source file:\n```typescript\n{}\n```\n\n\
             Extract the duplicated code into a shared utility function. \
             Create the utility in src/utils/shared-{}.ts. \
             Update the original file to import and use it. \
             Only output the shared utility file and the modified original file, \
             clearly labeled. Do NOT change any behavior.",
            block.locations.len(),
            first.file,
            first.start_line,
            first.end_line,
            block.snippet,
            block.locations[1..]
                .iter()
                .map(|l| format!("  - {} (lines {}-{})", l.file, l.start_line, l.end_line))
                .collect::<Vec<_>>()
                .join("\n"),
            source,
            block.id,
        );

        match chat(config, &system_prompt, &user_prompt).await {
            Ok(response) => {
                // Parse the response for file blocks.
                let files = parse_multi_file_response(&response);
                for (path_hint, content) in &files {
                    let out_path = project_root.join(path_hint);
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&out_path, content)?;
                    println!(
                        "  {} Written/updated {} ({} bytes)",
                        "✓".green(),
                        out_path.display(),
                        content.len(),
                    );
                    if path_hint.contains("shared-") {
                        result.shared_files_created.push(out_path);
                    } else {
                        result.files_modified.push(out_path);
                    }
                }
                result.blocks_refactored += 1;
            }
            Err(e) => {
                eprintln!("  {} AI error for block {}: {}", "✗".red(), block.id, e);
            }
        }
    }

    Ok(result)
}

struct DupBlock {
    id: String,
    snippet: String,
    line_count: usize,
    locations: Vec<DupLocation>,
}

struct DupLocation {
    file: String,
    start_line: usize,
    end_line: usize,
}

fn run_duplication_scan(root: &Path) -> Result<String> {
    // Run `lens scan --format json` and capture output.
    let output =
        std::process::Command::new(std::env::current_exe().context("getting current exe path")?)
            .arg("scan")
            .arg(root)
            .arg("--format")
            .arg("json")
            .arg("--quiet")
            .output()
            .context("running lens scan")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

fn parse_duplication_report(json: &str, min_lines: usize) -> Result<Vec<DupBlock>> {
    // Parse the JSON output from lens scan.
    let start = json.find('{').unwrap_or(0);
    let val: serde_json::Value =
        serde_json::from_str(&json[start..]).context("parsing scan output")?;
    let dup = val
        .get("duplication")
        .and_then(|d| d.get("blocks"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let mut blocks = Vec::new();
    if let Some(arr) = dup.as_array() {
        for (i, block) in arr.iter().enumerate() {
            let line_count = block
                .get("line_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            if line_count < min_lines {
                continue;
            }
            let snippet = block
                .get("snippet")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let locations: Vec<DupLocation> = block
                .get("locations")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|loc| {
                    Some(DupLocation {
                        file: loc.get("file")?.as_str()?.to_string(),
                        start_line: loc.get("start_line")?.as_u64()? as usize,
                        end_line: loc.get("end_line")?.as_u64()? as usize,
                    })
                })
                .collect();

            if locations.len() >= 2 {
                blocks.push(DupBlock {
                    id: format!("dup-{}", i),
                    snippet,
                    line_count,
                    locations,
                });
            }
        }
    }
    Ok(blocks)
}

fn build_dedup_system_prompt() -> String {
    "You are a code refactoring agent. Your job is to extract duplicated \
     TypeScript/JavaScript code into shared utility functions.\n\n\
     Rules:\n\
     1. NEVER change the behavior of the application\n\
     2. Extract the duplicated code into a shared utility function\n\
     3. Update all files that had the duplicate to import and use the shared function\n\
     4. Keep the same function signature and return type\n\
     5. Place shared utilities in src/utils/\n\
     6. Output each file clearly labeled with its path\n\
     7. Use the format: === FILE: path/to/file.ts === followed by the content"
        .to_string()
}

fn parse_multi_file_response(response: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut current_path = String::new();
    let mut current_content = String::new();

    for line in response.lines() {
        if line.starts_with("=== FILE:") || line.starts_with("**File:") {
            // Save previous file
            if !current_path.is_empty() {
                files.push((current_path.clone(), current_content.trim().to_string()));
            }
            // Parse new path
            current_path = line
                .trim_start_matches('=')
                .trim_start_matches('*')
                .trim()
                .trim_start_matches("FILE:")
                .trim()
                .trim_end_matches('=')
                .trim_end_matches('*')
                .trim()
                .to_string();
            current_content.clear();
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }
    if !current_path.is_empty() {
        files.push((current_path, current_content.trim().to_string()));
    }

    files
}

use owo_colors::OwoColorize;
