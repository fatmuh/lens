# Changelog

All notable changes to Lens will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **SonarQube-compatible duplication mode (Phase 5)**:
  - New line-based algorithm that detects 100% identical consecutive-line
    blocks appearing in 2+ files. Closer to SonarQube's `sonar.duplications`
    metric.
  - Configurable via `--sonar-compat` flag or `duplication.mode = "sonar"`
    in `quality-gate.toml`. The default mode (`"token"`) is unchanged.
  - `--min-duplicate-lines <N>` flag and `duplication.min_lines` config
    (default 100, matching SonarQube) control the block-size threshold.
  - Output labels clearly indicate which mode produced the numbers
    (`token-based: 21.90%` vs `sonar-compat (line-based): 0.11%`).
- 6 new unit tests for the SonarQube-compatible mode.

### Known differences from SonarQube
- We use a generic whitespace-normalized token hash; SonarQube uses a
  language-specific lexer with more aggressive identifier normalization.
- SonarQube includes "near-duplicate" matching (e.g., variable rename
  tolerance); we require exact line equality.
- As a result, our `sonar-compat` mode is more conservative than
  SonarQube's, typically reporting 5–20x lower percentages on the same
  codebase. It is best used as an order-of-magnitude approximation.

### Changed
- **`--normalize-identifiers` flag + `duplication.normalize_identifiers` config**:
  In `sonar-compat` mode, identifiers (a-zA-Z0-9_ starting with letter/_)
  are now replaced with the literal `@id` before per-line hashing. This
  makes the algorithm invariant to variable, function, and class renames
  and catches structurally-identical code that differs only by name. Off
  by default to preserve exact-hash semantics; turn on with
  `--normalize-identifiers` or `normalize_identifiers = true` in
  `quality-gate.toml`.

## [0.1.0] — 2026-06-06

## [0.1.0] — 2026-06-06

### Added
- **Foundation (Phase 0)**:
  - CLI built with `clap`, supporting `scan`, `init`, `rules`, `version` subcommands.
  - File discovery respecting `.gitignore` and `.lensignore`, plus include/exclude globs.
  - Language detection for 26 languages (Rust, TypeScript, JavaScript, TSX, JSX,
    Python, Go, Java, C, C++, C#, Ruby, PHP, Kotlin, Swift, Scala, Bash, SQL,
    HTML, CSS, SCSS, JSON, YAML, TOML, Markdown).
  - `quality-gate.toml` configuration loader with full schema.
  - Output formats: terminal, JSON, HTML, SARIF.
  - Forward-compatible JSON schema with `null` placeholders for future phases.
- **NOSONAR support (Phase 0)**:
  - Recognizes 5 comment styles: `// NOSONAR`, `# NOSONAR`, `-- NOSONAR`,
    `/* NOSONAR */`, `<!-- NOSONAR -->`.
  - Case-insensitive matching; respects language-specific comment syntax.
- **Polish (Phase 0.5)**:
  - Windows path normalization (strips `\\?\` extended-length prefix).
  - Progress bar with TTY detection.
  - Real SARIF 2.1.0 output for GitHub Code Scanning / GitLab Code Quality.
  - HTML report with stats cards, language breakdown, "What's next" roadmap.
  - `--quiet` and `--no-color` flags.
  - `--output` flag for all formats.
  - Duration tracking.
- **TypeScript metrics (Phase 1)**:
  - Per-file: LOC, code/comment/blank line counts, comment density.
  - Counts: functions, classes, interfaces, type aliases, enums.
  - Per-function cyclomatic complexity (with parameter count and line range).
  - Project-wide: total metrics, average complexity, top-N most complex functions.
  - Built on `tree-sitter` + `tree-sitter-typescript`.
- **Duplication detection (Phase 1)**:
  - Sonar-style algorithm: token shingling + winnowing.
  - Project-wide percentage, top duplicated files.
  - Language-agnostic (works on any source via regex-based tokenization).
- **Block-level duplication details (Phase 1.5)**:
  - For each top file pair, runs longest-common-substring DP on fingerprints.
  - Reports actual source line ranges (`file:start-end` for each occurrence).
  - Top 20 blocks by token count, with overlap dedup.
- **Coverage parsing (Phase 3)**:
  - **LCOV** format (`.info` files from Jest, nyc, karma, ...).
  - **Cobertura** XML format (Java/.NET projects).
  - **JaCoCo** XML format (Java default).
  - Honors `[coverage].exclude` patterns (e.g. `**/*.spec.ts`).
  - Top 10 files with lowest coverage, per-file uncovered-line lists.
- **Quality gate**:
  - `--gate` exits non-zero when configured thresholds are violated.
  - Checks both **duplication** (e.g. > 3% fails) and **coverage** (e.g. < 80% fails).
  - Reports which specific check failed when multiple are configured.
- **Distribution**:
  - `install.sh` for Linux/macOS (auto-detects platform, downloads release binary).
  - `install.ps1` for Windows (PowerShell one-liner, adds to PATH).
  - GitHub Actions for CI (fmt + clippy + test on 3 OSes) and release (5 targets).
  - Pre-built binaries for Linux (glibc + musl), macOS (Intel + Apple Silicon), Windows.

### Performance
- Scans 1,206 TypeScript files (292K LOC) in **~13 seconds** end-to-end on
  developer hardware.
- Parallelism via `rayon`. Block detection bounded to top 500 file pairs
  (most-impacted first).
- Coverage parsing is essentially free (file I/O + simple parsing).

[Unreleased]: https://github.com/fatmuh/lens/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/fatmuh/lens/releases/tag/v0.1.0
