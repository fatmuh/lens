# Lens

> **Lightweight, self-hosted code quality scanner** — issues, duplication, coverage.
> A single static binary alternative to SonarQube. No JVM, no DB, no server.

[![CI](https://github.com/fatmuh/lens/actions/workflows/ci.yml/badge.svg)](https://github.com/fatmuh/lens/actions/workflows/ci.yml)
[![Release](https://github.com/fatmuh/lens/actions/workflows/release.yml/badge.svg)](https://github.com/fatmuh/lens/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

```
$ lens scan .                                              [pos-glid-b2b — 1,206 files, 292K LOC]

  Lens scan results
  ──────────────────────────────────────────────────
  Root:     D:\Nutech\Backend\pos-glid-b2b
  Config:   D:\Nutech\Backend\pos-glid-b2b\quality-gate.toml
  Files:    1206
  NOSONAR:  3 marker(s)
  Duration: 13.18 s

  Metrics (TypeScript)
  ┌─────────────────────────────┬────────┐
  │ Total LOC                   │ 292427 │
  │ Functions                   │  18296 │
  │ Classes                     │    813 │
  │ Total cyclomatic complexity │  30391 │
  │ Avg complexity / function   │   1.66 │
  └─────────────────────────────┴────────┘
  🔥 Most complex: createCatalogFtlLtl() (CC=87, line 975)

  Duplication
  19.84% of 1361138 tokens are duplicated (67462 shared fingerprints)

  Top duplicate blocks:
    1111 tokens
      catalog.builder.spec.ts:1-192
      catalog.builder.spec.ts:16-209
    770 tokens
      order-mapping.helper.ts:93-225
      order-service.helper.ts:1772-1904

  Coverage
  0.68% of 99694 executable lines covered across 702 file(s) [format: lcov]

✗ Quality gate: FAIL (duplication 19.84% > 3.00%, coverage 0.68% < 80.00%)
```

---

## ✨ Why Lens?

SonarQube is great, but it's:
- **Heavy**: requires Java, a database, and a dedicated server.
- **Slow**: scans take minutes because of network round-trips and server-side rendering.
- **Vendor-tied**: the Community Edition has feature gaps, and the Developer Edition costs money.

Lens is the **developer-first alternative**:
- **Single static binary** — no runtime, no server, no DB.
- **Sub-second to 15-second scans** for projects up to 1M LOC.
- **Self-contained** — respects `.gitignore`, `.lensignore`, and `NOSONAR` comments.
- **CI-friendly** — JSON, SARIF, and HTML outputs out of the box.

---

## 🚀 Quick start

### Install

**Windows (PowerShell):**
```powershell
iwr -useb https://raw.githubusercontent.com/fatmuh/lens/main/install.ps1 | iex
```

**macOS / Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/fatmuh/lens/main/install.sh | sh
```

**Or via `cargo`:**
```bash
cargo install lens
```

**Or download a binary** from [GitHub Releases](https://github.com/fatmuh/lens/releases).

### Use

```bash
# 1. Create a starter config in your project
lens init

# 2. Scan the current directory
lens scan .

# 3. Get a JSON report (for CI)
lens scan . --format json

# 4. Generate an HTML report
lens scan . --format html --output ./lens-report

# 5. Enforce the quality gate (exit non-zero on failure)
lens scan . --gate

# 6. SARIF for GitHub Code Scanning / GitLab Code Quality
lens scan . --format sarif > lens.sarif

# 7. Use SonarQube-compatible (line-based) duplication detection
lens scan . --sonar-compat --min-duplicate-lines 100
```

---

## 📊 What it does

| Capability | Status | Notes |
|---|---|---|
| **Metrics** (LOC, complexity, function count) | ✅ | TypeScript/TSX (more languages in Phase 7) |
| **Duplication %** (project-wide) | ✅ | Sonar-style token shingling + winnowing |
| **Block-level duplication** (file:line ranges) | ✅ | Longest-common-substring DP on top file pairs |
| **Coverage parsing** | ✅ | LCOV, Cobertura XML, JaCoCo XML |
| **NOSONAR support** | ✅ | 5 comment styles, language-aware, case-insensitive |
| **Quality gate enforcement** | ✅ | Duplication threshold + coverage threshold |
| **HTML report** | ✅ | Single self-contained file, no external deps |
| **JSON output (CI-friendly)** | ✅ | Forward-compatible schema with placeholders |
| **SARIF 2.1.0 output** | ✅ | GitHub Code Scanning, GitLab Code Quality |
| **`.gitignore` / `.lensignore` respect** | ✅ | Includes glob whitelist, excludes blacklist |
| **Progress bar** | ✅ | TTY-aware, configurable via `--quiet` |
| **Rule engine (issues)** | 🚧 | Planned for Phase 2 |
| **Watch mode** | 🚧 | Planned for Phase 6 |

---

## ⚙️ Configuration

Lens reads `quality-gate.toml` from the scan root (or pass `--config <path>`).
See [`quality-gate.toml.example`](quality-gate.toml.example) for the full schema.

```toml
[scan]
exclude = ["**/node_modules/**", "**/dist/**", "**/coverage/**"]
include = ["src/**"]                # optional whitelist

[nosonar]
enabled = true
custom_markers = []                 # additional markers besides "NOSONAR"

[duplication]
min_tokens = 100                    # minimum block size
fail_above_percent = 3.0            # quality-gate threshold

[coverage]
report_paths = ["coverage/lcov.info", "**/cobertura-coverage.xml"]
fail_below_percent = 80.0
exclude = ["**/*.spec.ts", "**/*.test.ts"]   # don't count tests

[issues]                            # Phase 2 — placeholder
fail_on = ["blocker", "critical"]
```

### Ignoring files

Two layers, both respected:

1. **`.gitignore`** — auto-detected (disable with `--no-gitignore`).
2. **`.lensignore`** — same syntax as `.gitignore`. Useful for generated code
   or vendored deps that git legitimately tracks but you don't want scanned.

### NOSONAR

Lens respects the standard `NOSONAR` comment in any of these styles
(recognized per language):

| Style | Languages |
|---|---|
| `// NOSONAR` | C, C++, C#, Java, Go, JS, TS, Rust, Kotlin, Swift, Scala |
| `# NOSONAR` | Python, Ruby, Shell, YAML, TOML, PHP |
| `-- NOSONAR` | SQL, Lua, Haskell |
| `/* NOSONAR */` | CSS, SCSS, PHP |
| `<!-- NOSONAR -->` | HTML, XML |

Match is case-insensitive. Custom markers can be added via
`[nosonar].custom_markers`.

---

## 🤖 CI integration

### GitHub Actions

```yaml
- name: Install lens
  run: |
    curl -fsSL https://raw.githubusercontent.com/fatmuh/lens/main/install.sh | sh
    echo "$HOME/.cargo/bin" >> $GITHUB_PATH

- name: Scan
  run: lens scan . --gate

- name: Upload SARIF to GitHub Code Scanning
  if: always()
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: lens.sarif
```

### GitLab CI

```yaml
lens:
  script:
    - curl -fsSL https://raw.githubusercontent.com/fatmuh/lens/main/install.sh | sh
    - lens scan . --format sarif --output lens.sarif
    - lens scan . --gate
  artifacts:
    reports:
      codequality: lens.sarif
```

### JSON output for custom CI

```json
{
  "lens_version": "0.1.0",
  "scan": { "root": "...", "duration_ms": 13180 },
  "summary": {
    "total_files": 1206,
    "nosonar_markers": 3,
    "by_language": { "TypeScript": 1206 }
  },
  "metrics": {
    "total_loc": 292427,
    "functions": 18296,
    "cyclomatic_complexity": 30391,
    "avg_complexity_per_function": 1.66,
    "max_function": {
      "name": "createCatalogFtlLtl",
      "complexity": 87,
      "start_line": 975
    }
  },
  "duplication": {
    "duplication_percent": 19.84,
    "shared_fingerprint_count": 67462,
    "blocks": [
      {
        "token_count": 1111,
        "occurrences": [
          { "file": "src/.../catalog.builder.spec.ts", "start_line": 1,  "end_line": 192 },
          { "file": "src/.../catalog.builder.spec.ts", "start_line": 16, "end_line": 209 }
        ]
      }
    ]
  },
  "coverage": {
    "format": "lcov",
    "file_count": 702,
    "coverage_percent": 0.68,
    "files": [
      { "file": "src/app.module.ts", "total_lines": 146, "covered_lines": 0, "coverage_percent": 0.0 }
    ]
  },
  "issues": []
}
```

---

## 🔬 Sample HTML report

Open `lens-report/index.html` after running `lens scan . --format html`:

- **Stats cards** for files, LOC, functions, complexity, coverage %, duplication %.
- **Color-coded** thresholds (green / yellow / red).
- **Top 5 most complex functions** with clickable file paths.
- **Top 20 duplicate blocks** with `file:start_line-end_line` for each occurrence.
- **Top 10 files with lowest coverage**.
- Single self-contained HTML file with inline CSS — no external requests.

---

## 🆚 Lens vs SonarQube

| Feature | Lens 0.1.0 | SonarQube Community |
|---|---|---|
| Single static binary | **✅** | ❌ (JVM + DB + server) |
| Setup time | **< 1 minute** | 30+ minutes |
| Scan time (1M LOC) | **~15 seconds** | minutes (with server) |
| Metrics (LOC, complexity) | ✅ (TS) | ✅ (30+ languages) |
| Duplication % | ✅ | ✅ |
| Block-level duplication | ✅ | ✅ (web UI) |
| Coverage parsing | ✅ (LCOV, Cobertura, JaCoCo) | ✅ (many formats) |
| Quality gate | ✅ (dup + coverage) | ✅ (more rules) |
| Rule engine (issues) | 🚧 Phase 2 | ✅ |
| HTML report | ✅ | ✅ |
| JSON / SARIF output | ✅ | ✅ |
| Inline issue highlighting | 🚧 Phase 5+ | ✅ |
| Multi-language | TS, JS, TSX, JSX, +25 detected | 30+ |
| Self-hosted | **✅** | ✅ |
| Free | **✅** | ✅ (Community) / 💰 (Developer) |

**When to choose Lens**: developer workflow, single-machine CI, fast feedback, no infra.
**When to choose SonarQube**: enterprise deployment, multi-language rules, PR decoration.

---

## 🗺️ Roadmap

| Phase | Status | Feature |
|---|---|---|
| **0** | ✅ | Foundation: CLI, config, file discovery, NOSONAR |
| **0.5** | ✅ | Polish: path normalization, progress bar, HTML/SARIF, JSON schema |
| **1** | ✅ | TypeScript metrics + duplication % (shingling + winnowing) |
| **1.5** | ✅ | Block-level duplication with line ranges |
| **3** | ✅ | Coverage parsing (LCOV, Cobertura, JaCoCo) |
| **2** | 🚧 | Rule engine (~10 starter rules per language) |
| **4** | 🚧 | Interactive HTML (click-to-view code) |
| **5** | 🚧 | More languages (Rust, Python, Go, Java metrics) |
| **6** | 🚧 | Watch mode + monorepo support |
| **7** | 🚧 | Plugin system for custom rules |
| **8** | 🚧 | Inline source highlighting (LSP integration) |

---

## 🛠️ Build from source

```bash
git clone https://github.com/fatmuh/lens
cd lens
cargo build --release
./target/release/lens --version
```

Run tests:
```bash
cargo test                       # all tests
cargo test --bin lens            # unit tests only
cargo test --test integration    # integration tests only
```

Run with sample data:
```bash
mkdir /tmp/lens-demo
cat > /tmp/lens-demo/main.ts <<'EOF'
function add(a: number, b: number): number {
    if (a > 0) return a + b;
    return b;
}
EOF
./target/release/lens scan /tmp/lens-demo
```

---

## 🤝 Contributing

Contributions welcome! See [`CONTRIBUTING.md`](CONTRIBUTING.md) for development
setup, code style, and the PR process.

For bug reports and feature requests, please open an issue on
[GitHub](https://github.com/fatmuh/lens/issues).

---

## 📄 License

[MIT](LICENSE) © 2026 Lens Contributors
