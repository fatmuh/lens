//! Test runner — auto-detect test framework, run tests with coverage,
//! parse results, and feed into the AI coverage agent.
//!
//! Supported frameworks:
//!   - Jest (most common in TS/JS projects)
//!   - Vitest (Vite-native, Jest-compatible)
//!   - Mocha (via nyc for coverage)
//!
//! Detection: reads `package.json` scripts + checks for config files.

use anyhow::{Context, Result};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Detected test framework and its configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFramework {
    pub name: String,
    pub config_file: Option<String>,
    pub test_cmd: Vec<String>,
    pub coverage_cmd: Vec<String>,
    pub coverage_output_dir: String,
    pub coverage_format: String,
}

/// Result of running tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResult {
    pub success: bool,
    pub total_tests: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub duration_ms: u64,
    pub coverage_file: Option<PathBuf>,
    pub coverage_percent: f64,
    pub output: String,
    /// Per-test results parsed from test output.
    pub test_cases: Vec<TestCaseResult>,
}

/// A single test case result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseResult {
    pub name: String,
    pub status: TestCaseStatus,
    pub file: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TestCaseStatus {
    Passed,
    Failed,
    Skipped,
    Todo,
}

/// Detect the test framework used in a project.
pub fn detect(root: &Path) -> Result<TestFramework> {
    let pkg_path = root.join("package.json");
    if !pkg_path.exists() {
        anyhow::bail!("No package.json found. Only Node.js projects are supported.");
    }

    let pkg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&pkg_path).context("reading package.json")?
    ).context("parsing package.json")?;

    let deps = pkg.get("dependencies").cloned().unwrap_or_default();
    let dev_deps = pkg.get("devDependencies").cloned().unwrap_or_default();
    let scripts = pkg.get("scripts").cloned().unwrap_or_default();

    // Merge deps + devDeps for checking.
    let has_dep = |name: &str| -> bool {
        deps.get(name).is_some() || dev_deps.get(name).is_some()
    };

    // Check for vitest first (explicit, usually preferred if installed).
    if has_dep("vitest") {
        return Ok(TestFramework {
            name: "vitest".into(),
            config_file: find_config(root, &["vitest.config.ts", "vitest.config.js"]),
            test_cmd: build_npx_cmd("vitest", "run", &scripts, "test"),
            coverage_cmd: build_npx_cmd("vitest", "run --coverage", &scripts, "test:cov"),
            coverage_output_dir: "coverage".into(),
            coverage_format: "lcov".into(),
        });
    }

    // Check for jest.
    if has_dep("jest") || has_dep("ts-jest") {
        let jest_config = find_config(root, &[
            "jest.config.ts", "jest.config.js", "jest.config.mjs",
            "jest.config.cjs",
        ]);

        // Build coverage command.
        let mut cov_cmd = build_npx_cmd("jest", "--coverage", &scripts, "test:cov");

        // If there's a specific config, add it.
        if let Some(cfg) = &jest_config {
            // Check if the scripts already reference the config
            let already_has_config = cov_cmd.iter().any(|arg| arg.contains("jest.config"));
            if !already_has_config {
                cov_cmd.push(format!("--config={}", cfg));
            }
        }

        // Detect coverage output from jest config or default.
        let coverage_dir = detect_coverage_dir(root, &jest_config);

        return Ok(TestFramework {
            name: "jest".into(),
            config_file: jest_config,
            test_cmd: build_npx_cmd("jest", "", &scripts, "test"),
            coverage_cmd: cov_cmd,
            coverage_output_dir: coverage_dir,
            coverage_format: "lcov".into(),
        });
    }

    // Check for mocha.
    if has_dep("mocha") {
        return Ok(TestFramework {
            name: "mocha".into(),
            config_file: find_config(root, &[".mocharc.yml", ".mocharc.json", ".mocharc.js"]),
            test_cmd: build_npx_cmd("mocha", "", &scripts, "test"),
            coverage_cmd: build_npx_cmd("nyc", "mocha", &scripts, "test:cov"),
            coverage_output_dir: "coverage".into(),
            coverage_format: "lcov".into(),
        });
    }

    anyhow::bail!(
        "Could not detect test framework. Found dependencies: {}\n\
         Supported: jest, vitest, mocha. Add one to your devDependencies.",
        list_deps(&deps, &dev_deps)
    )
}

/// Run the test suite (without coverage).
pub fn run_tests(root: &Path, framework: &TestFramework) -> Result<TestRunResult> {
    let cmd = if framework.test_cmd.is_empty() {
        &framework.coverage_cmd
    } else {
        &framework.test_cmd
    };
    execute_test_command(root, cmd)
}

/// Run the test suite WITH coverage.
pub fn run_with_coverage(root: &Path, framework: &TestFramework) -> Result<TestRunResult> {
    let result = execute_test_command(root, &framework.coverage_cmd)?;

    // Find the coverage output file.
    let coverage_file = find_coverage_output(root, &framework.coverage_output_dir);

    let mut result = result;
    result.coverage_file = coverage_file;

    // Parse coverage percent from LCOV if available.
    if let Some(ref lcov) = result.coverage_file {
        if let Ok(pct) = parse_lcov_percent(lcov) {
            result.coverage_percent = pct;
        }
    }

    Ok(result)
}

fn execute_test_command(root: &Path, args: &[String]) -> Result<TestRunResult> {
    if args.is_empty() {
        anyhow::bail!("No test command configured");
    }

    use owo_colors::OwoColorize;
    let program = &args[0];
    let cmd_args = &args[1..];

    let start = std::time::Instant::now();

    // Print header.
    eprintln!();
    eprintln!("  {} Running: {} {}", "->".dimmed(), program.cyan(), cmd_args.join(" ").cyan());
    eprintln!();

    // Spawn child with stdout/stderr inherited so jest output streams directly.
    let mut child = std::process::Command::new(program)
        .args(cmd_args)
        .current_dir(root)
        .env("FORCE_COLOR", "1")
        .env("CI", "true")
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .context(format!("Failed to run {}. Is it installed?", program))?;

    // Wait in a background thread so we can show a spinner.
    let handle = std::thread::spawn(move || {
        let status = child.wait();
        (status, start.elapsed().as_millis() as u64)
    });

    // Show spinner while waiting.
    let spinner = ["|", "/", "-", "\\"];
    let mut idx = 0usize;
    let mut dots = 0usize;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        // Check if done (non-blocking).
        if handle.is_finished() { break; }
        idx += 1;
        dots = (dots + 1) % 4;
        let dot_str = ".".repeat(dots + 1);
        eprint!("\r  {} Running tests {} {:>3} ", "\u{1F9EA}".cyan(), dot_str, spinner[idx % 4]);
        let _ = std::io::Write::flush(&mut std::io::stderr());
    }

    let (status_result, duration) = handle.join()
        .map_err(|_| anyhow::anyhow!("Test process thread panicked"))?;
    let status = status_result.context("Waiting for test process")?;

    // Clear spinner line.
    eprint!("\r{}\r", " ".repeat(50));
    let _ = std::io::Write::flush(&mut std::io::stderr());

    let success = status.success();

    // Try to parse test summary from jest coverage-summary.json.
    let summary_path = root.join("coverage").join("coverage-summary.json");
    let (total, passed, failed, skipped, test_cases) = if summary_path.exists() {
        parse_jest_summary(&summary_path)
    } else {
        (0, 0, 0, 0, vec![])
    };

    Ok(TestRunResult {
        success,
        total_tests: total,
        passed,
        failed,
        skipped,
        duration_ms: duration,
        coverage_file: None,
        coverage_percent: 0.0,
        output: String::new(),
        test_cases,
    })
}

/// Parse jest's coverage-summary.json for test counts.
/// This file contains per-file coverage data but NOT individual test results.
/// For individual tests, we'd need --json output, but that conflicts with --coverage.
/// So we infer: total lines = total tests (rough), covered = passed, etc.
fn parse_jest_summary(path: &Path) -> (u32, u32, u32, u32, Vec<TestCaseResult>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (0, 0, 0, 0, vec![]),
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return (0, 0, 0, 0, vec![]),
    };

    // coverage-summary.json has structure:
    // { "total": { "lines": { "total": N, "covered": C, ... } }, "src/foo.ts": { ... } }
    // We use line coverage as a proxy.
    let total_lines = val.get("total")
        .and_then(|t| t.get("lines"))
        .and_then(|l| l.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let covered_lines = val.get("total")
        .and_then(|t| t.get("lines"))
        .and_then(|l| l.get("covered"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    // Build per-file test cases from coverage data.
    let mut test_cases = Vec::new();
    if let Some(files) = val.as_object() {
        for (file_name, file_data) in files {
            if file_name == "total" { continue; }
            let pct = file_data.get("lines")
                .and_then(|l| l.get("pct"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            test_cases.push(TestCaseResult {
                name: file_name.clone(),
                status: if pct >= 100.0 { TestCaseStatus::Passed }
                        else if pct > 0.0 { TestCaseStatus::Failed }
                        else { TestCaseStatus::Skipped },
                file: Some(file_name.clone()),
                duration_ms: None,
            });
        }
    }

    // Return line-level summary as test counts (rough approximation).
    // Actual test counts come from jest's own output which goes to terminal.
    (total_lines, covered_lines, total_lines - covered_lines, 0, test_cases)
}

fn parse_test_summary(output: &str) -> (u32, u32, u32, u32) {
    // Jest: "Tests:       5 passed, 2 failed, 1 skipped, 8 total"
    // Vitest: same format
    // Mocha: "passing (5)", "failing (2)"
    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;

    // Try Jest/Vitest format first.
    for line in output.lines() {
        let line = line.to_lowercase();
        if line.contains("tests:") && (line.contains("passed") || line.contains("total")) {
            // "Tests:       5 passed, 2 failed, 1 skipped, 8 total"
            if let Some(n) = extract_count(&line, "passed") { passed = n; }
            if let Some(n) = extract_count(&line, "failed") { failed = n; }
            if let Some(n) = extract_count(&line, "skipped") { skipped = n; }
            if let Some(n) = extract_count(&line, "total") { total = n; }
        }
    }

    // Fallback: if no Jest format found, try Mocha.
    if total == 0 {
        for line in output.lines() {
            let line = line.to_lowercase();
            if line.contains("passing") {
                if let Some(n) = extract_count(&line, "passing") { passed = n; }
            }
            if line.contains("failing") {
                if let Some(n) = extract_count(&line, "failing") { failed = n; }
            }
        }
        total = passed + failed;
    }

    // If still nothing, infer from exit code.
    if total == 0 && passed == 0 && failed == 0 {
        // Just set total = 1 if output had content
        total = 0;
    }

    (total, passed, failed, skipped)
}

/// Parse individual test case results from Jest/Vitest output.
///
/// Jest verbose output looks like:
///   PASS unit-test/foo.spec.ts (5.234 s)
///     ✓ should do something (3 ms)
///     ✕ should fail (1 ms)
///
///   FAIL unit-test/bar.spec.ts (2.123 s)
///     ✓ should work (1 ms)
///
/// Also handles the summary section:
///   PASS src/__tests__/app.service.spec.ts
///   FAIL src/__tests__/auth.guard.spec.ts
fn parse_test_cases(output: &str) -> Vec<TestCaseResult> {
    let mut cases = Vec::new();
    let mut current_file: Option<String> = None;

    for line in output.lines() {
        let trimmed = line.trim();

        // Detect file header: "PASS unit-test/foo.spec.ts (5 s)" or "FAIL ..."
        // or "PASS src/foo.spec.ts"
        if trimmed.starts_with("PASS ") || trimmed.starts_with("FAIL ") {
            let is_pass = trimmed.starts_with("PASS");
            let rest = &trimmed[5..]; // after "PASS " or "FAIL "
            // Strip duration: "unit-test/foo.spec.ts (5.234 s)"
            let file = if let Some(paren_pos) = rest.find(" (") {
                &rest[..paren_pos]
            } else {
                rest
            };
            current_file = Some(file.trim().to_string());
            continue;
        }

        // Detect individual test: "✓ should do something (3 ms)"
        //   or "✕ should fail (1 ms)"
        //   or "○ skipped test"
        //   or "● should fail"
        if let Some(rest) = trimmed.strip_prefix('✓')
            .or_else(|| trimmed.strip_prefix('✅'))
            .or_else(|| trimmed.strip_prefix("PASS"))
        {
            let (name, duration) = parse_test_line(rest.trim());
            cases.push(TestCaseResult {
                name,
                status: TestCaseStatus::Passed,
                file: current_file.clone(),
                duration_ms: duration,
            });
        } else if let Some(rest) = trimmed.strip_prefix('✕')
            .or_else(|| trimmed.strip_prefix('✗'))
            .or_else(|| trimmed.strip_prefix('✘'))
            .or_else(|| trimmed.strip_prefix('●'))
            .or_else(|| trimmed.strip_prefix("FAIL"))
        {
            let (name, duration) = parse_test_line(rest.trim());
            cases.push(TestCaseResult {
                name,
                status: TestCaseStatus::Failed,
                file: current_file.clone(),
                duration_ms: duration,
            });
        } else if let Some(rest) = trimmed.strip_prefix('○')
            .or_else(|| trimmed.strip_prefix('◌'))
            .or_else(|| trimmed.strip_prefix("SKIP"))
        {
            let (name, duration) = parse_test_line(rest.trim());
            cases.push(TestCaseResult {
                name,
                status: TestCaseStatus::Skipped,
                file: current_file.clone(),
                duration_ms: duration,
            });
        }
    }

    cases
}

/// Parse a test line like "should do something (3 ms)" → (name, Some(3))
fn parse_test_line(s: &str) -> (String, Option<u64>) {
    // Try to extract duration from trailing "(N ms)" or "(N s)"
    if let Some(paren_start) = s.rfind(" (") {
        let name = s[..paren_start].trim().to_string();
        let duration_str = &s[paren_start + 2..];
        // "3 ms)" or "5.234 s)"
        if let Some(end) = duration_str.find(')') {
            let dur_part = &duration_str[..end];
            if let Some(ms_str) = dur_part.strip_suffix(" ms") {
                if let Ok(ms) = ms_str.parse() {
                    return (name, Some(ms));
                }
            } else if let Some(s_str) = dur_part.strip_suffix(" s") {
                if let Ok(secs) = s_str.parse::<f64>() {
                    return (name, Some((secs * 1000.0) as u64));
                }
            }
        }
    }
    (s.trim().to_string(), None)
}

fn extract_count(line: &str, keyword: &str) -> Option<u32> {
    // Find keyword, extract number before it.
    // "5 passed" → 5
    let keyword_lower = keyword;
    if let Some(pos) = line.find(keyword_lower) {
        let before = &line[..pos];
        // Walk backwards to find the number.
        let num_str: String = before.chars().rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars().rev().collect();
        return num_str.parse().ok();
    }
    None
}

fn find_config(root: &Path, candidates: &[&str]) -> Option<String> {
    for name in candidates {
        if root.join(name).exists() {
            return Some(name.to_string());
        }
    }
    None
}

/// Build an npx command, preferring the npm script if it exists.
fn build_npx_cmd(
    tool: &str,
    extra_args: &str,
    scripts: &serde_json::Value,
    preferred_script: &str,
) -> Vec<String> {
    // Check if there's a script for this.
    if let Some(script) = scripts.get(preferred_script) {
        if let Some(s) = script.as_str() {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if !parts.is_empty() {
                let first = parts[0];
                if first == "npx" || first == "yarn" || first == "pnpm" || first == "bun" {
                    let mut cmd = vec![resolve_cmd(first)];
                    cmd.extend(parts[1..].iter().map(|s| s.to_string()));
                    return cmd;
                }
                // Otherwise, run via npx
                let mut cmd = vec![resolve_cmd("npx")];
                cmd.extend(parts.iter().map(|s| s.to_string()));
                return cmd;
            }
        }
    }

    // Fallback: npx <tool> <extra_args>
    let mut cmd = vec![resolve_cmd("npx"), tool.to_string()];
    if !extra_args.is_empty() {
        cmd.extend(extra_args.split_whitespace().map(|s| s.to_string()));
    }
    cmd
}

/// On Windows, `npx` is `npx.cmd`. Resolve to the correct binary.
fn resolve_cmd(cmd: &str) -> String {
    #[cfg(windows)]
    {
        // Check if the .cmd version exists.
        let cmd_ext = format!("{}.cmd", cmd);
        if which_exists(&cmd_ext) {
            return cmd_ext;
        }
        // Fallback to the base command.
        cmd.to_string()
    }
    #[cfg(not(windows))]
    {
        cmd.to_string()
    }
}

fn which_exists(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok()
}

fn detect_coverage_dir(root: &Path, jest_config: &Option<String>) -> String {
    // Default jest coverage dir is "coverage".
    // Check if package.json has collectCoverageFrom with a custom dir.
    let _ = (root, jest_config);
    "coverage".into()
}

fn find_coverage_output(root: &Path, dir: &str) -> Option<PathBuf> {
    let cov_dir = root.join(dir);
    if !cov_dir.exists() {
        return None;
    }

    // Look for LCOV file in common locations.
    let candidates = [
        "lcov.info",
        "lcov.info.txt",
        "lcov.info.json",
    ];

    // Check directly in coverage dir.
    for name in &candidates {
        let path = cov_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }

    // Check subdirs (jest sometimes puts it in coverage/lcov-report/lcov.info).
    if let Ok(entries) = std::fs::read_dir(&cov_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                for name in &candidates {
                    let p = path.join(name);
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
    }

    None
}

fn parse_lcov_percent(path: &Path) -> Result<f64> {
    let content = std::fs::read_to_string(path)?;
    let mut total = 0u64;
    let mut covered = 0u64;

    for line in content.lines() {
        if let Some(da) = line.strip_prefix("DA:") {
            let parts: Vec<&str> = da.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(hits) = parts[1].parse::<u64>() {
                    total += 1;
                    if hits > 0 {
                        covered += 1;
                    }
                }
            }
        }
    }

    if total == 0 {
        return Ok(0.0);
    }

    Ok((covered as f64 / total as f64) * 100.0)
}

fn list_deps(
    deps: &serde_json::Value,
    dev_deps: &serde_json::Value,
) -> String {
    let mut names: Vec<String> = Vec::new();
    if let Some(obj) = deps.as_object() {
        names.extend(obj.keys().cloned());
    }
    if let Some(obj) = dev_deps.as_object() {
        names.extend(obj.keys().cloned());
    }
    names.sort();
    names.dedup();
    names.join(", ")
}
