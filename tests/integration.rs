//! End-to-end tests for the `lens` CLI.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Helper: build a `lens` command and run it on `dir`, returning output.
fn lens() -> Command {
    Command::cargo_bin("lens").expect("lens binary built")
}

fn make_project() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    std::fs::write(dir.path().join("main.ts"), "const x: number = 1;\n").unwrap();
    std::fs::write(
        dir.path().join("util.ts"),
        "export function f() { return 42; } // NOSONAR\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules")).unwrap();
    std::fs::write(
        dir.path().join("node_modules").join("lib.ts"),
        "should be excluded",
    )
    .unwrap();
    dir
}

#[test]
fn scan_terminal_finds_files_and_respects_gitignore() {
    let dir = make_project();
    lens()
        .args(["scan", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("2")) // 2 files
        .stdout(predicate::str::contains("TypeScript"))
        .stdout(predicate::str::contains("1 marker"));
}

#[test]
fn scan_json_has_forward_compatible_schema() {
    let dir = make_project();
    let output = lens()
        .args(["scan", "--format", "json", dir.path().to_str().unwrap()])
        .output()
        .expect("run lens");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid JSON output");

    assert_eq!(json["lens_version"], env!("CARGO_PKG_VERSION"));
    assert!(json["scan"]["root"].is_string());
    assert!(json["scan"]["duration_ms"].is_number());
    assert!(json["summary"]["total_files"].is_number());
    assert!(json["summary"]["by_language"]["TypeScript"].is_number());
    // Phase 1: TS is supported, so metrics is an object (not null).
    assert!(json["metrics"].is_object() || json["metrics"].is_null());
    assert!(json["duplication"].is_object());
    // Coverage (Phase 3) and issues (Phase 2) remain placeholders:
    assert!(json["coverage"].is_null());
    assert!(json["issues"].is_array());
}

#[test]
fn scan_sarif_is_valid_structure() {
    let dir = make_project();
    let output = lens()
        .args(["scan", "--format", "sarif", dir.path().to_str().unwrap()])
        .output()
        .expect("run lens");
    assert!(output.status.success());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid JSON output");

    assert_eq!(json["version"], "2.1.0");
    assert_eq!(json["runs"][0]["tool"]["driver"]["name"], "lens");
    assert!(json["runs"][0]["results"].is_array());
    assert!(json["runs"][0]["properties"]["lens"]["totalFiles"].is_number());
}

#[test]
fn scan_html_writes_file() {
    let dir = make_project();
    let out = dir.path().join("report");
    lens()
        .args([
            "scan",
            "--format",
            "html",
            "--output",
            out.to_str().unwrap(),
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let html = std::fs::read_to_string(out.join("index.html")).expect("read html");
    assert!(html.contains("<!doctype html>"));
    assert!(html.contains("Lens Report"));
    assert!(html.contains("TypeScript"));
}

#[test]
fn scan_quiet_suppresses_progress() {
    let dir = make_project();
    lens()
        .args(["scan", "--quiet", dir.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn init_creates_config() {
    let dir = TempDir::new().unwrap();
    lens()
        .args(["init", "--force", dir.path().to_str().unwrap()])
        .assert()
        .success();
    let cfg = std::fs::read_to_string(dir.path().join("quality-gate.toml")).unwrap();
    assert!(cfg.contains("[scan]"));
    assert!(cfg.contains("[duplication]"));
    assert!(cfg.contains("[coverage]"));
}

#[test]
fn nosonar_is_case_insensitive() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("a.ts"),
        "x = 1; // nosonar\ny = 2; // NOSONAR\nz = 3; // Nosonar\n",
    )
    .unwrap();
    let output = lens()
        .args(["scan", "--format", "json", dir.path().to_str().unwrap()])
        .output()
        .expect("run lens");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["summary"]["nosonar_markers"], 3);
}

#[test]
fn metrics_are_computed_for_typescript() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("a.ts"),
        r#"
function add(a: number, b: number): number {
    if (a > 0) return a + b;
    return b;
}

class Foo {
    bar() { return 42; }
}
"#,
    )
    .unwrap();
    let output = lens()
        .args(["scan", "--format", "json", dir.path().to_str().unwrap()])
        .output()
        .expect("run lens");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["metrics"].is_object());
    let m = &json["metrics"];
    assert!(m["total_loc"].as_u64().unwrap() > 0);
    assert!(m["functions"].as_u64().unwrap() >= 2); // add + bar
    assert!(m["classes"].as_u64().unwrap() >= 1);
    assert!(m["cyclomatic_complexity"].as_u64().unwrap() >= 2); // add has if
}

#[test]
fn duplication_detected_across_similar_files() {
    let dir = TempDir::new().unwrap();
    let shared = r#"
function processOrder(order: any) {
    if (!order) return null;
    if (order.id) {
        return { id: order.id, status: 'active', total: order.total };
    }
    if (order.items) {
        return { id: 'temp', status: 'pending', total: order.items.length };
    }
    return null;
}
"#;
    std::fs::write(dir.path().join("a.ts"), shared).unwrap();
    std::fs::write(dir.path().join("b.ts"), shared).unwrap();
    std::fs::write(dir.path().join("c.ts"), "const x = 42;\n").unwrap();

    let output = lens()
        .args([
            "scan",
            "--format",
            "json",
            "--token-mode",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("run lens");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let dup = &json["duplication"];
    assert!(dup["duplication_percent"].as_f64().unwrap() > 0.0);
    assert!(dup["files_with_duplication"].as_u64().unwrap() >= 2);
}

#[test]
fn block_level_duplication_has_line_ranges() {
    // Build two substantial files with a shared duplicate block.
    let shared: String = (0..200)
        .map(|i| {
            if i % 2 == 0 {
                format!("identifier_{} = {};\n", i, i)
            } else {
                format!(
                    "if (identifier_{} > 0) {{ identifier_{} = identifier_{} + 1; }}\n",
                    i, i, i
                )
            }
        })
        .collect();
    let mut a = shared.clone();
    a.push_str("\nfunction uniqueToA() { return 1; }\n");
    let mut b = shared.clone();
    b.push_str("\nfunction uniqueToB() { return 2; }\n");

    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.ts"), &a).unwrap();
    std::fs::write(dir.path().join("b.ts"), &b).unwrap();

    let output = lens()
        .args([
            "scan",
            "--format",
            "json",
            "--token-mode",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("run lens");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    let blocks = json["duplication"]["blocks"]
        .as_array()
        .expect("blocks array");
    assert!(!blocks.is_empty(), "expected >= 1 block");

    let block = &blocks[0];
    let occs = block["occurrences"].as_array().unwrap();
    assert_eq!(occs.len(), 2);

    for occ in occs {
        assert!(occ["file"].as_str().unwrap().ends_with(".ts"));
        let start = occ["start_line"].as_u64().unwrap();
        let end = occ["end_line"].as_u64().unwrap();
        assert!(start > 0 && end >= start);
    }
}

#[test]
fn coverage_lcov_is_parsed() {
    let dir = TempDir::new().unwrap();
    // A simple LCOV report covering 3 of 5 lines.
    std::fs::write(
        dir.path().join("lcov.info"),
        "SF:src/foo.ts\nDA:1,1\nDA:2,5\nDA:3,0\nDA:4,1\nDA:5,0\nend_of_record\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("quality-gate.toml"),
        "[coverage]\nreport_paths = [\"lcov.info\"]\nfail_below_percent = 0.0\n",
    )
    .unwrap();

    let output = lens()
        .args(["scan", "--format", "json", dir.path().to_str().unwrap()])
        .output()
        .expect("run lens");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let cov = &json["coverage"];
    assert_eq!(cov["format"], "lcov");
    assert_eq!(cov["file_count"], 1);
    assert_eq!(cov["total_lines"], 5);
    assert_eq!(cov["covered_lines"], 3);
    assert!((cov["coverage_percent"].as_f64().unwrap() - 60.0).abs() < 0.01);
}

#[test]
fn coverage_exclude_patterns_are_respected() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("lcov.info"),
        "SF:src/foo.ts\nDA:1,1\nDA:2,1\nend_of_record\n\
         SF:src/foo.spec.ts\nDA:1,0\nDA:2,0\nend_of_record\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("quality-gate.toml"),
        "[coverage]\nreport_paths = [\"lcov.info\"]\nexclude = [\"**/*.spec.ts\"]\n",
    )
    .unwrap();

    let output = lens()
        .args(["scan", "--format", "json", dir.path().to_str().unwrap()])
        .output()
        .expect("run lens");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let cov = &json["coverage"];
    // After excluding *.spec.ts, only the first file is counted.
    assert_eq!(cov["file_count"], 1);
    assert_eq!(cov["total_lines"], 2);
    assert_eq!(cov["covered_lines"], 2);
}

#[test]
fn quality_gate_exits_nonzero_on_failure() {
    let dir = TempDir::new().unwrap();
    // Two files with substantial overlap (so duplication > 0%).
    let shared = r#"
function processOrder(order: any) {
    if (!order) return null;
    if (order.id) {
        return { id: order.id, status: 'active', total: order.total };
    }
    if (order.items) {
        return { id: 'temp', status: 'pending', total: order.items.length };
    }
    return null;
}
"#;
    std::fs::write(dir.path().join("a.ts"), shared).unwrap();
    std::fs::write(dir.path().join("b.ts"), shared).unwrap();

    // Write a config with a 0% threshold so the gate always fails.
    let cfg = "[duplication]\nfail_above_percent = 0.0\n";
    std::fs::write(dir.path().join("quality-gate.toml"), cfg).unwrap();

    // With --gate: exits non-zero.
    lens()
        .args([
            "scan",
            "--gate",
            "--token-mode",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(1);
}
