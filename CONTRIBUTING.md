# Contributing to Lens

Thanks for your interest in contributing! 🎉

## Development setup

1. Install Rust (1.75 or newer) via [rustup](https://rustup.rs/).
2. Clone the repo:
   ```bash
   git clone https://github.com/fatmuh/lens
   cd lens
   ```
3. Build and test:
   ```bash
   cargo build
   cargo test
   ```

## Code style

- **Format** with `cargo fmt` before committing (CI will fail otherwise).
- **Lint** with `cargo clippy --all-targets -- -D warnings`.
- Prefer small, focused commits. Use `git rebase -i` to clean up before pushing.
- Add tests for new features — we have a mix of unit tests (in `src/`) and
  end-to-end tests (in `tests/integration.rs`).

## Project layout

```
src/
├── main.rs             CLI entry point
├── cli.rs              clap definitions
├── config.rs           config loading & `init` subcommand
├── util/               small helpers
├── scanner/            file discovery, language detection, NOSONAR, report glue
├── analyzer/           metrics + duplication (Phase 1)
├── coverage/           LCOV / Cobertura / JaCoCo parsers (Phase 3)
└── report/             (placeholder for future HTML refactor)
tests/
└── integration.rs      end-to-end CLI tests
```

When adding a new feature, prefer to put it in its own module under
`analyzer/` (or another focused subdir) rather than expanding an existing
file.

## Adding a new language

1. Add the `Language` variant in `src/scanner/language.rs` with its file
   extensions and NOSONAR comment style.
2. Add a `tree-sitter-<lang>` dependency to `Cargo.toml`.
3. Wire the parser in `src/analyzer/parser.rs::get_language`.
4. If the language has metrics rules of its own, add them to
   `src/analyzer/metrics.rs::compute`.

## Adding a new rule (Phase 2)

Rules will live under `src/analyzer/rules/<language>.rs` and implement a
common `Rule` trait. See `src/scanner/rules.rs` for the (currently stubbed)
registry.

## Adding a new coverage format

Add a new file in `src/coverage/`, implement a `parse(content: &str) ->
CoverageReport` function, and register the format detector in
`src/coverage/mod.rs::detect_and_parse`.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/) where
possible:

```
feat: add Rust language support
fix: handle missing end_of_record in LCOV
docs: improve coverage section in README
refactor: extract path normalization to util module
test: add integration test for SARIF output
chore: bump dependencies
```

## Release process

1. Bump version in `Cargo.toml`.
2. Update `CHANGELOG.md` with the new version section.
3. Commit, push, and tag: `git tag v0.X.Y && git push --tags`.
4. GitHub Actions will build binaries for all 5 targets and create a draft
   release. Review, edit notes if needed, then publish.

## Questions?

Open an issue or start a discussion on GitHub. We're friendly!

