# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

ccsesh is a Rust CLI that lists and resumes recent Claude Code sessions by reading JSONL log files from `~/.claude/projects/`. Single binary, no runtime dependencies.

## Build & Test Commands

```sh
cargo build                          # Build debug binary
cargo test                           # Run all tests (unit + integration)
cargo test --lib                     # Unit tests only
cargo test --test integration        # Integration tests only
cargo test <test_name>               # Run a single test by name
cargo install --path .               # Install to ~/.cargo/bin/
```

Uses Rust 2024 edition. CI enforces `cargo fmt --check` and `cargo clippy -- -D warnings` — run both before pushing.

Run locally without installing:
```sh
cargo run                            # List sessions (uses real ~/.claude/projects/)
cargo run -- --json --limit 3        # Pass CLI args after --
cargo run -- init fish               # Test shell init output
```

## Architecture

Two-phase pipeline: **discover** (stat-only, no file reads) then **parse** (read top 50 lines of selected files).

```
main.rs  →  discover.rs  →  parse.rs  →  display.rs
   ↓                                        ↑
 shell.rs                               types.rs, errors.rs
```

- **main.rs** — Clap derive CLI. Dispatches: no args → list sessions, `init` → print shell wrapper, numeric → resume session. Hidden `--shell-mode` flag for shell wrapper protocol.
- **discover.rs** — `discover_sessions()` walks `~/.claude/projects/*/`, stats `.jsonl` files, sorts by mtime descending, truncates to `--limit`. No file content is read.
- **parse.rs** — `parse_session()` reads up to 50 lines per file extracting `cwd`, `slug`, and first qualifying user prompt. Three-layer prompt filter: skip `isMeta`, skip `isCompactSummary`, skip slash commands (after XML tag stripping). `strip_xml_tags()` is a hand-written char scanner (no regex crate) with newline bail-out for literal `<`.
- **display.rs** — Three formatters: `format_default` (aligned columns with header/footer), `format_short` (compact, no chrome), `format_json` (pretty JSON, no truncation). Colors via `owo-colors` with TTY/`NO_COLOR` detection.
- **shell.rs** — Shell wrapper templates (fish/bash/zsh) and `__CCSESH_EXEC__` exec protocol. UUID validation at the shell boundary before eval.
- **types.rs** — `SessionCandidate` (path+mtime), `SessionInfo` (full parsed data), `JsonlLine` (serde deserializer with camelCase renames), `OutputFormat` enum.
- **errors.rs** — `CcseshError` enum via thiserror. Domain errors bubble up through anyhow at the top level.

## Key Design Decisions

- **mtime over JSONL timestamps** — ranking uses filesystem mtime from stat(), avoiding the need to open every file just to sort.
- **50-line parse cap** — cwd/slug/prompt appear near file top; `MAX_LINES = 50` in parse.rs bounds I/O.
- **Skip-on-failure** — individual unparseable files are silently skipped; only total failure raises `NoSessionsFound`.
- **No regex crate** — XML stripping is a simple bracket matcher; newline before `>` means the `<` was literal content.
- **Option\<String\> positional args** — first arg can be "init", a number, or absent; clap subcommands don't naturally handle the numeric-index case.
- **Duplicate `is_valid_uuid`** — exists in both parse.rs and shell.rs intentionally; the shell.rs copy is a security check before eval.

## Testing

- Unit tests live in `#[cfg(test)] mod tests` within each module.
- **tests/fixtures/** contains 11 synthetic JSONL files covering edge cases (meta-only, empty, XML markup, slash commands, array content, image paste, compact summary, no cwd, summary-only, truncated).
- Integration tests (tests/integration.rs) use `assert_cmd` + `assert_fs::TempDir` with overridden `$HOME` and `NO_COLOR=1`. Fixtures are copied with deterministic UUIDs and controlled mtimes.
- Parse unit tests copy fixtures to temp files with UUID filenames since `parse_session` validates filename format.

**Adding a new fixture:** Create the `.jsonl` file in `tests/fixtures/`, then add a UUID mapping in `fixture_to_uuid()` in both `tests/integration.rs` and use `fixture_candidate()` helper in `parse.rs` unit tests. The filename must be a valid lowercase UUID.

## Commit Conventions

Use [Conventional Commits](https://www.conventionalcommits.org/): `type: description`. Common types: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`. Keep the subject line under 72 characters, imperative mood.

## Distribution

See `docs/distribution.md` for full setup guide. Release workflow: bump version in Cargo.toml, tag `vX.Y.Z`, push — CI builds all targets and updates Homebrew tap.

- `.github/workflows/ci.yml` — lint + test on push/PR
- `.github/workflows/release.yml` — build + release on `v*` tags
- `install.sh` / `install.ps1` — curl/irm-pipeable install scripts
- `homebrew-tap/` — Homebrew formula (copied to separate `homebrew-ccsesh` repo)
- `winget/` — winget manifest files (submitted to microsoft/winget-pkgs)
