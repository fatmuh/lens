# Lens

> **Lightweight, self-hosted code quality & security scanner** — issues, duplication, coverage, dependencies, DAST.
> A single static binary alternative to SonarQube. No JVM, no DB, no server.

[![CI](https://github.com/fatmuh/lens/actions/workflows/ci.yml/badge.svg)](https://github.com/fatmuh/lens/actions/workflows/ci.yml)
[![Release](https://github.com/fatmuh/lens/actions/workflows/release.yml/badge.svg)](https://github.com/fatmuh/lens/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

```
$ lens scan .

  🔍 discovered 47 file(s)

  Lens scan results
  ────────────────────────────────────────────────────────────
  Root:     ./my-project
  Config:   ./my-project/quality-gate.toml
  Files:    47
  Duration: 0.18 s

  By language:
  ┌──────────────┬───────┐
  │ Language     │ Files |
  ╞══════════════╪═══════╡
  │ TypeScript   │    38 │
  │ TypeScript   │     4 │
  │ Dart         │     3 │
  │ Go           │     2 │
  └──────────────┴───────┘

  Metrics
  ┌─────────────────────────────┬────────┐
  │ Total LOC                   │   4812 │
  │ Functions                   │    312 │
  │ Classes                     │     24 │
  │ Total cyclomatic complexity │    418 │
  │ Avg complexity / function   │   1.34 │
  └─────────────────────────────┴────────┘

  Duplication
  sonar-compat (line-based): 0.00% of 0 lines are duplicated

  Issues
  0 blocker, 0 critical, 3 major, 12 minor, 4 info

  Top violated rules:
      3  rust/avoid-unwrap
      5  no-console
      4  go/avoid-print

  ℹ 610 rules, 4 languages, custom rules, security taint analysis,
    SonarQube-compatible duplication, and coverage parsing.
```

---

## ✨ Why Lens?

SonarQube is great, but it's:

- **Heavy**: requires Java, a database, and a dedicated server.
- **Slow**: scans take minutes because of network round-trips and server-side rendering.
- **Vendor-tied**: the Community Edition has feature gaps, and paid editions cost money.

Lens is the **developer-first alternative**:

- **Single static binary** — no runtime, no server, no DB. Just download and run.
- **Sub-second to 15-second scans** for projects up to 1M LOC.
- **610 rules** across 4 languages, with custom rules support.
- **Dependency scanning** — checks npm, crates.io, Go, and Pub packages against the OSV database.
- **DAST scanning** — OWASP ZAP integration for dynamic security testing.
- **AI-powered auto-fix** — fix issues, generate tests, refactor duplicates.
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

**Or download a binary** from [GitHub Releases](https://github.com/fatmuh/lens/releases).

**Or self-update** (after initial install):
```bash
lens update          # Download and install latest version
lens update --check  # Just check, don't install
```

### Use

```bash
# Initialize config
lens init

# Scan for issues, duplication, coverage
lens scan .

# JSON output for CI
lens scan . --format json

# HTML report
lens scan . --format html --output ./lens-report

# Enforce quality gate
lens scan . --gate

# SARIF for GitHub Code Scanning
lens scan . --format sarif > lens.sarif
```

---

## 📊 What it does

| Capability | Details |
|---|---|
| **4 languages** | TypeScript/JS, Dart/Flutter, Go, Rust |
| **610 rules** | 99 TS/JS + 6 Dart + 6 Go + 6 Rust built-in + 493 SonarJS metadata |
| **Custom rules** | Regex-based rules in `quality-gate.toml` |
| **Metrics** | LOC, complexity, function count, classes, enums, interfaces |
| **Duplication** | SonarQube-compatible line-based detection |
| **Coverage** | LCOV, Cobertura XML, JaCoCo XML parsing |
| **Security taint analysis** | SQL injection, XSS, SSRF, path traversal, prototype pollution |
| **Dependency scanning** | OSV database (npm, crates.io, Go, Pub) |
| **DAST scanning** | OWASP ZAP integration |
| **AI auto-fix** | BYOK OpenAI-compatible API (Ollama supported) |
| **NOSONAR support** | 5 comment styles, language-aware |
| **CI/CD** | SARIF output, quality gate, PR comments |

---

## 🛡️ Security features

### Dependency scanning (`lens dep`)

Check your dependencies against the [OSV database](https://osv.dev) for known CVEs:

```bash
lens dep .                # Scan all lock files
lens dep . --gate         # Fail CI on critical vulns
lens dep . --format json  # Machine-readable output
```

Supports: `package-lock.json`, `Cargo.lock`, `go.sum`, `pubspec.lock`

```
$ lens dep .

  🔍 Found 324 dependencies across 1 lock file(s)
  🌐 Querying OSV database...

  ⚠️ Vulnerable dependencies
  ────────────────────────────────────────────────────────────
  🔴 GHSA-r6hc-6qgp-3cxw Improper Input Validation [npm]
    🔗 https://osv.dev/vulnerability/GHSA-r6hc-6qgp-3cxw
  🟠 RUSTSEC-2024-0384 instant is unmaintained [crates.io]
    🔗 https://osv.dev/vulnerability/RUSTSEC-2024-0384

  ⚠️ 0 critical, 0 high, 0 medium, 0 low, 2 info
```

### Dynamic scanning (`lens zap`)

Scan running web applications with OWASP ZAP:

```bash
lens zap http://localhost:3000             # Auto-start ZAP + scan
lens zap http://localhost:3000 --no-docker  # Use existing ZAP
lens zap http://localhost:3000 --gate       # Fail CI on vulns
```

### Security taint analysis

Built-in intra-procedural taint tracking detects:

| Language | Vulnerability classes |
|----------|---------------------|
| TypeScript/JS | SQL Injection, XSS, SSRF, Command Injection, Path Traversal, Prototype Pollution, Open Redirect, Log Injection |
| Dart/Flutter | Flutter SQL Injection, Flutter HTTP Injection, Flutter Path Traversal |

---

## 🌍 Language support

### TypeScript / JavaScript (99 rules)

Full AST-based metrics, 99 built-in rules, security taint analysis, and SonarJS metadata layer (493 stubs).

### Dart / Flutter (6 rules)

AST metrics, CPD tokenizer, Flutter-specific rules. Test files (`_test.dart`) excluded from duplication.

### Go (6 rules)

AST metrics including structs, interfaces, and type declarations. Error checking, doc comment enforcement.

### Rust (6 rules)

Lens scans its own source code. Structs, enums, traits, closures. Nested block comments and raw strings handled.

### Custom rules

Define your own regex-based rules in `quality-gate.toml`:

```toml
[[rules.custom]]
id = "no-hardcoded-secrets"
name = "No hardcoded secrets"
severity = "blocker"
languages = ["typescript", "javascript", "dart"]
pattern = '''sk-[a-zA-Z0-9]{20,}'''
message = "Hardcoded secret detected. Use environment variables instead."
```

---

## 🤖 AI-powered auto-fix

Lens can fix issues, generate tests, and refactor duplicates using an OpenAI-compatible API:

```bash
# Configure AI settings
lens setup

# Fix issues
lens fix .

# Generate tests
lens test .

# Preview changes without writing
lens fix . --dry-run
```

Works with any OpenAI-compatible API including local models via [Ollama](https://ollama.ai).

---

## ⚙️ Configuration

Lens reads `quality-gate.toml` from the scan root (or pass `--config <path>`).

```toml
[scan]
exclude = ["**/node_modules/**", "**/dist/**", "**/.dart_tool/**"]
include = ["src/**", "lib/**"]

[duplication]
mode = "sonar"
min_tokens = 100

[coverage]
report_paths = ["coverage/lcov.info"]
fail_below_percent = 80.0

[rules]
disabled = ["no-console", "no-magic-numbers"]

# Custom regex rules
[[rules.custom]]
id = "no-todo-without-ticket"
name = "TODO must have ticket"
severity = "info"
pattern = '''TODO(?!.*#\d+)'''
message = "TODO should reference a ticket (e.g. TODO #1234)"
```

### Ignoring files

1. **`.gitignore`** — auto-detected (disable with `--no-gitignore`).
2. **`.lensignore`** — same syntax, for files you track in git but don't want scanned.

### NOSONAR

Suppress issues on specific lines:

```typescript
result.unwrap(); // NOSONAR
```

| Style | Languages |
|---|---|
| `// NOSONAR` | C, C++, C#, Go, Java, JS, Rust, TS, Kotlin, Swift, Scala, Dart |
| `# NOSONAR` | Python, Ruby, Shell, YAML, TOML, PHP |
| `-- NOSONAR` | SQL, Lua, Haskell |
| `/* NOSONAR */` | CSS, SCSS, PHP |
| `<!-- NOSONAR -->` | HTML, XML |

---

## 🤝 CI/CD integration

### `lens ci` — All-in-one CI command

```bash
lens ci . --gate --pr-comment
```

Outputs SARIF, runs quality gate, and optionally generates a PR comment.

### GitHub Actions

```yaml
- name: Install Lens
  run: |
    curl -fsSL https://raw.githubusercontent.com/fatmuh/lens/main/install.sh | sh
    echo "$HOME/.cargo/bin" >> $GITHUB_PATH

- name: Scan
  run: lens ci . --gate

- name: Upload SARIF
  if: always()
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: lens-results.sarif
```

### Dependency scanning in CI

```yaml
- name: Check dependencies
  run: lens dep . --gate
```

### GitLab CI

```yaml
lens:
  script:
    - curl -fsSL https://raw.githubusercontent.com/fatmuh/lens/main/install.sh | sh
    - lens ci . --gate --output lens-results.sarif
  artifacts:
    reports:
      codequality: lens-results.sarif
```

---

## 🆚 Lens vs SonarQube

| Feature | Lens v0.9 | SonarQube Community |
|---|---|---|
| **Setup** | Download binary → done | JVM + PostgreSQL + server |
| **Scan time** | ~15 seconds | minutes (with server) |
| **Languages** | TS/JS, Dart, Go, Rust | 30+ languages |
| **Rules** | 610 + custom regex | 3000+ (per language) |
| **Security taint analysis** | ✅ (8 TS + 3 Dart vuln classes) | ✅ |
| **Dependency scanning** | ✅ (OSV database) | ✅ (via plugins) |
| **DAST scanning** | ✅ (OWASP ZAP) | ❌ |
| **AI auto-fix** | ✅ (BYOK) | ❌ |
| **Custom rules** | ✅ (regex in config) | ✅ (Java plugins) |
| **Quality gate** | ✅ | ✅ |
| **Duplication** | ✅ (SonarQube-compatible) | ✅ |
| **Coverage** | ✅ (LCOV, Cobertura, JaCoCo) | ✅ |
| **HTML report** | ✅ | ✅ |
| **SARIF output** | ✅ | ✅ |
| **Self-hosted** | ✅ | ✅ |
| **Free** | ✅ | ✅ (Community) / 💰 (Developer) |
| **Binary size** | ~4 MB | ~300 MB |

**When to choose Lens**: developer workflow, fast CI, no infra, AI-powered fixes, DAST scanning.
**When to choose SonarQube**: enterprise deployment, 30+ language support, web UI, PR decoration.

---

## 🗺️ Roadmap

| Feature | Status |
|---|---|
| **4 languages** (TS/JS, Dart, Go, Rust) | ✅ |
| **610 rules** + custom regex rules | ✅ |
| **SonarQube-compatible duplication** | ✅ |
| **Coverage parsing** (LCOV, Cobertura, JaCoCo) | ✅ |
| **Security taint analysis** | ✅ |
| **Dependency scanning** (OSV) | ✅ |
| **OWASP ZAP integration** (DAST) | ✅ |
| **AI auto-fix** (BYOK) | ✅ |
| **Self-update** (`lens update`) | ✅ |
| **CI command** (`lens ci`) | ✅ |
| GitHub App (PR bot) | 🔜 |
| VS Code extension | 🔜 |
| Python support | 🔜 |
| Java/Kotlin support | Planned |

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
cargo test
```

---

## 🤝 Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and PR process.

For bug reports and feature requests, open an issue at [GitHub Issues](https://github.com/fatmuh/lens/issues).

---

## 📄 License

[MIT](LICENSE) © 2026 Fatmuh
