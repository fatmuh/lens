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

    let program = &args[0];
    let cmd_args = &args[1..];

    use owo_colors::OwoColorize;
    println!("  {} Running: {} {}", "→".dimmed(), program.cyan(), cmd_args.join(" ").cyan());

    let start = std::time::Instant::now();
    let output = std::process::Command::new(program)
        .args(cmd_args)
        .current_dir(root)
        .env("FORCE_COLOR", "0")       // disable colors for parsing
        .env("CI", "true")             // run mode (no watch)
        .output()
        .context(format!("Failed to run {}. Is it installed?", program))?;

    let duration = start.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}\n{}", stdout, stderr);
    let success = output.status.success();

    // Parse test summary from output.
    let (total, passed, failed, skipped) = parse_test_summary(&combined);

    Ok(TestRunResult {
        success,
        total_tests: total,
        passed,
        failed,
        skipped,
        duration_ms: duration,
        coverage_file: None,
        coverage_percent: 0.0,
        output: combined,
    })
}

/// Parse test summary from Jest/Vitest/Mocha output.
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
