# Changelog

All notable changes to Lens will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] — 2026-06-07

### Added
- **Per-rule configuration** — configurable thresholds in `quality-gate.toml`
  - `max_function_lines` (default 50)
  - `max_function_complexity` (default 15)
  - `max_params` (default 5)
  - `no_magic_numbers_min` (default 3)
  - `disabled` array to turn off specific rules

- **Incremental scanning** — skip files unchanged since last scan
  - Uses `.lens/state.json` file hashes
  - `⚡ scanned 5 of 4854 files (4849 skipped, hash match)`
  - Massive speedup for repeated scans on large codebases

- **Significant code filtering** — separate production vs test/generated issues
  - Default excludes: `*.test.ts`, `*.spec.ts`, `__tests__/`, `*.d.ts`, `dist/`, `build/`
  - `10,307 in significant code, 9,718 in test/generated (excluded from gate)`
  - Quality gate uses significant code only

- **AI-powered auto-fix agent** (`lens fix`, `lens watch`)
  - OpenAI-compatible BYOK — works with OpenAI, Ollama, Groq, OpenRouter, Together, any compatible API
  - Coverage agent: reads LCOV, generates Jest tests for uncovered lines
  - Dedup agent: finds duplicate blocks, refactors into shared utilities
  - Constraint: never changes existing application behavior

- **Interactive AI setup** (`lens setup`)
  - Prompts for API base URL (shows examples)
  - API key input (optional for local models)
  - Fetches models from `/models` endpoint
  - Interactive model selection (numbered list or custom name)
  - Saves to `~/.lens/config.toml`
  - Masked API key display: `sk-t...7890`

- **Test runner** (`lens test`)
  - Auto-detects test framework (Jest, Vitest, Mocha)
  - Finds config files (jest.config.ts, vitest.config.ts, etc.)
  - Runs tests with coverage — streams output real-time
  - Animated spinner while waiting for test output
  - Parses coverage-summary.json for per-file results
  - `lens test . --fix` feeds coverage gaps to AI agent
  - `lens test . --detect-only` shows framework without running

### Changed
- `max-function-lines`, `max-function-complexity`, `max-params`, `no-magic-numbers` now read thresholds from config
- Rules with configurable thresholds use `with_threshold()` constructor
- `RuleRegistry::with_config()` builds rules with user settings

## [0.2.0] — 2026-06-07

### Added
- **Rule engine (Phase 2)** — **65 hand-rolled + 493 SonarJS-compatible rule stubs = 556 rules total**

  **v0.2.0 — SonarQube parity (Tier 1–5)**:

  Tier 1 — New Code Period:
  - State tracking via `.lens/state.json` (file hashes + issue snapshots)
  - Per-file status: ADDED | CHANGED | UNCHANGED
  - CLI flag: `--new-code` to filter issues to only new code
  - CLI flag: `--no-state` for read-only scans (CI, dry-runs)

  Tier 2 — Multi-Coverage:
  - CLI flags: `--coverage-ut GLOB`, `--coverage-it GLOB`
  - Config keys: `[coverage].ut_paths`, `[coverage].it_paths`
  - CoverageReport now has `ut_*`, `it_*`, `new_*` fields
  - Output: separate `ut_coverage` and `it_coverage` percentages

  Tier 3 — Issue Lifecycle:
  - Tracks issue identity (rule+line+message) across scans
  - Per-issue status: NEW | PERSISTENT | FIXED | REGRESSED
  - Report line: "X new, Y persistent, Z fixed, W regressed"

  Tier 4 — Cognitive Complexity (S3776):
  - `src/analyzer/cognitive.rs` — own implementation based on the
    public Cognitive Complexity whitepaper (G. Ann Campbell, 2018)
  - Nesting penalty: +1 per nesting level for control flow structures
  - Counts if/else/ternary/switch/for/while/catch/&&/||/??/recursion
  - `max-function-complexity` rule now uses CC instead of cyclomatic
  - Default threshold raised to 15 (matching SonarJS S3776)

  Tier 5 — Quality Gate (new-code aware):
  - New gate: `new_coverage` (only fires if state exists)
  - Informational: `ut_coverage`, `it_coverage`
  - Rating gates: A–E based on issue density (reliability/security/maintainability)

  **SonarJS 1:1 compatibility layer**:
  - 493 SonarJS rules (S100–S6772) recognized with their S-ID, title,
    severity, and type — listed in `lens rules` for compatibility
  - Auto-generated from upstream SonarJS rule JSON (Apache 2.0) via
    `scripts/gen_sonar_rules.py` → `src/rules/sonar_compat.rs`
  - 65 hand-rolled rules (the most impactful) are the firing version;
    428 SonarJS-only stubs are no-ops
  - Users can write `quality-gate.toml` rules using SonarQube S-IDs
    (e.g. `S1523` = `no-eval`)
  that produce actionable `Issue`s with file:line location and a one-line
  fix suggestion. Each rule is a separate file in `src/rules/builtin/`,
  implementing the [`Rule`](src/rules/mod.rs) trait:
    - `no-explicit-any` (major) — flags `any` type annotations
    - `no-implicit-any` (minor) — flags untyped function parameters
    - `no-console` (minor) — flags `console.*` calls (skips test files)
    - `no-var` (major) — flags `var` declarations
    - `no-eqeqeq` (major) — flags `==` / `!=` (use `===` / `!==`)
    - `prefer-const` (minor) — flags `let` that is never reassigned
    - `no-unused-vars` (major) — flags unused parameters and locals
    - `no-magic-numbers` (info) — flags non-{0,1,2,10,100,1000} literals
    - `no-throw-literal` (major) — flags `throw "string"` (use `Error`)
    - `no-empty-function` (minor) — flags functions with empty bodies
    - `no-unreachable` (critical) — flags code after return/throw/break
    - `max-function-lines` (major) — flags functions > 50 lines
    - `max-function-complexity` (major) — flags CC > 10
    - `max-params` (major) — flags functions with > 5 parameters

  **Added 18 more rules** (security + correctness + style):
    - `no-eval` (blocker) — `eval()` is a code-injection vector
    - `no-new-func` (blocker) — `new Function(...)` is `eval` in disguise
    - `no-script-url` (critical) — `javascript:` URLs execute code
    - `no-html-link` (critical) — `dangerouslySetInnerHTML` is XSS-prone
    - `no-async-promise-executor` (critical) — `new Promise(async ...)`
    - `no-unsafe-finally` (critical) — `return`/`throw` in `finally` swallows errors
    - `no-fallthrough` (critical) — switch cases without `break`
    - `no-dupe-keys` (critical) — duplicate object literal keys
    - `no-self-compare` (major) — `x === x` is almost always a bug
    - `no-duplicate-imports` (major) — same module imported twice
    - `require-await` (major) — `async` functions without `await`
    - `no-promise-all-in-loop` (minor) — sequential awaits inside a loop
    - `prefer-template` (minor) — string concat → template literal
    - `no-useless-concat` (minor) — `'a' + 'b'` → `'ab'`
    - `no-negated-condition` (minor) — `if (!x) ...; else ...` → swap branches
    - `no-lonely-if` (minor) — `else { if (...) ... }` → `else if`
    - `no-nested-ternary` (minor) — nested ternaries
    - `no-unneeded-ternary` (minor) — `x ? true : false` → `x`

  **Added 17 more rules** (TS-specific + common bugs + more style):

  Security:
    - `no-implied-eval` (critical) — `setTimeout("code", n)` is implicit eval
    - `no-prototype-builtins` (critical) — `obj.hasOwnProperty(...)` is unsafe

  Correctness:
    - `no-redeclare` (critical) — top-level `const x` twice
    - `default-case` (major) — `switch` should have a `default`

  TypeScript-specific:
    - `no-non-null-assertion` (minor) — `!` non-null assertion
    - `prefer-nullish-coalescing` (minor) — `??` over `||`
    - `prefer-optional-chain` (minor) — `?.` over `&&` chains
    - `consistent-type-imports` (minor) — `import type` for type-only

  Best practices:
    - `no-import-assign` (major) — imports are read-only
    - `no-param-reassign` (major) — don't reassign parameters
    - `no-return-await` (minor) — `return await x` is unnecessary
    - `no-await-in-loop` (minor) — `await` in loop is sequential

  Style:
    - `prefer-arrow-callback` (minor) — arrow over function expression
    - `no-useless-return` (minor) — bare `return;` at end
    - `no-else-return` (minor) — `else` after `return` is redundant
    - `no-useless-rename` (minor) — `import { x as x }`
    - `no-new-buffer` (major) — `new Buffer()` is deprecated

  **Added 16 more rules** (security + correctness + naming + style):

  Security:
    - `no-with` (critical) — `with` statement (forbidden in strict mode)
    - `no-proto` (major) — `__proto__` (use `Object.getPrototypeOf`)
    - `no-new-symbol` (major) — `new Symbol()` throws (use `Symbol()`)
    - `no-control-regex` (major) — ASCII control chars in regex patterns

  Correctness:
    - `no-bitwise` (major) — `&`/`|`/`^`/`~` in regular code
    - `no-extra-bind` (minor) — unnecessary `.bind(this)`
    - `no-extra-boolean-cast` (minor) — `!!x` or `Boolean(x)` on already-boolean
    - `no-misused-new` (major) — `new` on an interface (use a class)
    - `no-sparse-arrays` (major) — `[1, , 3]` (use `undefined`)
    - `prefer-promise-reject-errors` (major) — `Promise.reject("str")` (use Error)
    - `no-empty-interface` (major) — `interface Foo {}` (use `type`)

  Naming:
    - `camelcase` (minor) — non-camelCase vars (allow `UPPER_CASE`)
    - `no-underscore-dangle` (minor) — trailing `_` in names

  Style:
    - `prefer-spread` (minor) — `[].concat(a, b)` → `[...a, ...b]`
    - `quote-props` (minor) — object keys with special chars
    - `no-warning-comments` (info) — `TODO`/`FIXME` without owner

  - **Output**: issues shown in terminal (severity glyphs + counts by
    severity + top violated rules + top 10 issues), in JSON output
    (`issues[]` + `summary.issues_by_severity`), and in SARIF
    (`runs[].results[]` with rule + location + level).
  - **Quality gate integration**: `--gate` now fails when there's > 0
    `blocker` or `critical` issues (in addition to duplication +
    coverage thresholds).
  - **`lens rules`** subcommand: lists all built-in rules with id,
    name, severity, languages, and (with `-v`) description. Supports
    `--language` filter and `--format json`.
- **Rule tests**: 65 new unit tests in `src/rules/rule_tests.rs`,
  one per rule (plus registry sanity checks). 155/155 total tests
  passing (142 unit + 13 integration).

## [0.1.1] — 2026-06-07

### Added
- **SonarQube-compatible duplication mode (Phase 5+)**:
  - Line-based algorithm that detects identical consecutive-statement
    blocks across 2+ files. Translated from `SonarSource/sonarqube`'s
    `sonar-duplications` module.
  - **Consecutive-duplicate filter** (from `BlockChunker.java`):
    collapses 3+ identical statements to first & last; 2 identical
    to first. Removes boilerplate noise before hashing.
  - **Rabin-Karp rolling hash** with base 31 (also from
    `BlockChunker.java`): `s[0]*31^9 + s[1]*31^8 + ... + s[9]`, with
    the classic O(N) rolling update.
  - **File-pair clone detection**: for every pair of files, count
    shared block hashes. If ≥ `min_blocks_per_file` (2 by default),
    report a clone spanning the bounding line range of the matches.
  - **Clone merging**: blocks referring to the same set of files are
    merged into a single report (so 3 files sharing a clone give 1
    report, not 3).
  - Configurable via `--sonar-compat` flag or
    `duplication.mode = "sonar"` in `quality-gate.toml`. The default
    mode (`"token"`) is unchanged.
  - `--min-duplicate-lines <N>` flag and `duplication.min_lines`
    config (default **250**, verified to match office SonarQube's
    output within < 0.02% on pos-glid-b2b).
  - Output labels clearly indicate which mode produced the numbers
    (`token-based: 21.90%` vs `sonar-compat (line-based): 2.51%`).
- **`--normalize-identifiers` flag + `duplication.normalize_identifiers` config**:
  In `sonar-compat` mode, identifiers (a-zA-Z0-9_ starting with letter/_)
  are now replaced with the literal `@id` before per-line hashing. This
  makes the algorithm invariant to variable/function renames and catches
  structurally-identical blocks that differ only by name. Off by default
  to preserve exact-hash semantics; turn on with `--normalize-identifiers`
  or `normalize_identifiers = true` in `quality-gate.toml`.

### Changed
- **Default `min_lines` bumped from 100 → 250** (SonarQube parity):
  Empirically, `min_lines=250` matches what office SonarQube reports
  for typical TypeScript codebases (verified on pos-glid-b2b: 2.51%
  vs office 2.5%, a difference of < 0.02%). The previous default of
  100 was over-reporting small duplicates (7.26% with `min=100`).
  SonarQube's own configuration knob is `minimumTokens=100`, but the
  combination of the consecutive-duplicate filter (BlockChunker.java)
  and Rabin-Karp block size of 10 statements makes the *effective*
  minimum much higher in practice. Set `duplication.min_lines = 100`
  in `quality-gate.toml` (or pass `--min-duplicate-lines 100` on the
  CLI) for the old sensitive behavior.

### Fixed
- **Non-deterministic duplication detection** (sonar-compat mode):
  The `sonar-compat` mode was using `HashMap<usize, HashMap<u64, ...>>`
  to group blocks by file. Rust's `HashMap` uses a random seed for DoS
  protection, which made file-pair processing order non-deterministic
  across runs. On pos-glid-b2b, this caused the duplication percentage
  to vary between runs (1.7% on one run, 2.5% on another). Switched
  to `BTreeMap` for sorted, deterministic iteration. The result is
  now identical on every run.

### Tests
- 55/55 passing (42 unit + 13 integration).
- New unit tests for the SonarQube algorithm: collapse filter edge
  cases, file-pair detection with 2/3 files, identifier normalization
  with renamed variables, below-threshold and intra-file filtering.

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

[Unreleased]: https://github.com/fatmuh/lens/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/fatmuh/lens/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/fatmuh/lens/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/fatmuh/lens/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/fatmuh/lens/releases/tag/v0.1.0
