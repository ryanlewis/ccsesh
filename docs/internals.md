# ccsesh Internals

## Overview

ccsesh is a Rust CLI that lists and resumes Claude Code sessions by reading JSONL log files from `~/.claude/projects/`. It uses a two-phase approach: first, cheaply discover session files via filesystem stat calls and sort by mtime; then, parse only the top-N files to extract metadata. This avoids reading the contents of potentially hundreds of large JSONL files when only a handful are displayed.

## Data Flow

```
  $HOME/.claude/projects/*/*.jsonl
                |
                v
  +---------------------------+
  | discover_sessions()       |
  |  - enumerate project dirs |
  |  - stat() each .jsonl     |
  |  - sort by mtime desc     |
  |  - truncate to --limit    |
  +---------------------------+
                |
        Vec<SessionCandidate>
          (path + mtime only)
                |
                v
  +---------------------------+
  | parse_session()           |   (called per candidate)
  |  - validate UUID filename |
  |  - read up to 50 lines    |
  |  - extract cwd, slug,     |
  |    first qualifying prompt |
  |  - substitute $HOME -> ~  |
  +---------------------------+
                |
           SessionInfo
                |
        +-------+--------+
        |       |        |
        v       v        v
  format_   format_  format_
  default   short    json
        |       |        |
        v       v        v
            stdout
```

## Module-by-Module Breakdown

### main.rs -- CLI entry point

Defines the `Cli` struct using clap derive. The `run()` function reads `$HOME`, then dispatches on the first positional argument: `None` lists sessions, `"init"` prints shell wrapper code, and anything else is parsed as a numeric index for session resume. Errors are printed to stderr with exit code 1.

Key types and functions:
- `Cli` -- clap-derived struct with `command: Option<String>`, `shell: Option<String>`, `--limit`, `--format`, `--json`, and a hidden `--shell-mode`.
- `run()` -- main logic, returns `anyhow::Result<()>`.
- `main()` -- catches errors from `run()`, prints to stderr, exits 1.

Notable: `--shell-mode` is `hide = true` in clap so it does not appear in `--help`. It is an internal flag used by shell wrapper functions.

### types.rs -- Shared data types

Defines the core structs that flow between modules.

- `SessionCandidate` -- lightweight pre-parse struct holding only `path: PathBuf` and `mtime: SystemTime`. Produced by discovery, consumed by parsing.
- `SessionInfo` -- fully parsed session: `session_id`, `path`, `project_dir`, `project_dir_display`, `last_active: DateTime<Utc>`, `first_prompt: Option<String>`, `slug: Option<String>`. Derives `Serialize` for JSON output.
- `JsonlLine` -- loosely-typed serde deserializer for a single JSONL line. Uses `#[serde(rename = ...)]` for camelCase fields (`isMeta`, `isCompactSummary`, `sessionId`). The `message.content` field is `Option<serde_json::Value>` to handle both string and array payloads.
- `JsonlMessage` -- nested struct holding `content: Option<serde_json::Value>`.
- `OutputFormat` -- clap `ValueEnum` with variants `Default` and `Short`.
- `shell_escape_single_quote()` -- wraps a string in single quotes, replacing internal `'` with `'\''`.

### errors.rs -- Error types

A single `CcseshError` enum using `thiserror::Error`:

| Variant | When raised |
|---------|-------------|
| `HomeDirectoryNotFound` | `$HOME` env var missing |
| `ProjectsDirNotFound { path }` | `~/.claude/projects/` does not exist |
| `NoSessionsFound` | Projects dir exists but no parseable JSONL files |
| `IndexOutOfRange { index, max }` | Resume index exceeds discovered session count |
| `SessionReadError { path, source }` | I/O error reading a session file (defined but currently unused) |
| `SessionParseError { path, detail }` | Parse failure for a session file (defined but currently unused) |
| `UnknownShell { shell }` | `ccsesh init <shell>` with unsupported shell name |

The unused variants exist as reserved extension points.

### discover.rs -- Session discovery (Phase 1)

`discover_sessions(home_dir: &str, limit: usize) -> Result<Vec<SessionCandidate>>`

Enumerates `{home_dir}/.claude/projects/*/` looking for `.jsonl` files at the top level of each project subdirectory. For each file it calls `metadata()` to get the mtime, pushes a `SessionCandidate`, then sorts all candidates by mtime descending and truncates to `limit`.

Implementation details:
- `limit == 0` returns `Ok(vec![])` immediately without any filesystem I/O.
- Non-JSONL files, directories, and nested subdirectories are silently skipped.
- Any individual I/O error (unreadable file, unreadable directory entry) is silently skipped via `continue`; only the absence of the projects directory itself is a hard error.
- Sorts with `b.mtime.cmp(&a.mtime)` for descending order.

### parse.rs -- JSONL parsing (Phase 2)

`parse_session(candidate: &SessionCandidate, home_dir: &str) -> Result<SessionInfo>`

Opens the session file with `BufReader`, reads up to `MAX_LINES` (50) lines, and extracts three fields:

1. **cwd** -- first `cwd` field found on any line type.
2. **slug** -- first `slug` field found on any line type.
3. **first_prompt** -- first qualifying user message (see extraction rules below).

Terminates early if all three are found before hitting the 50-line limit. Malformed JSON lines are silently skipped.

Other key functions:

- `extract_text_from_content(value)` -- handles the two content formats Claude Code uses: a plain string, or an array of `{"type":"text","text":"..."}` / `{"type":"image",...}` objects. Returns the first `text` item found.
- `strip_xml_tags(input)` -- character-level scanner that removes `<...>` sequences. If a newline appears before the closing `>`, the `<` is treated as literal content (preserving things like `< 10 mins`). After stripping, whitespace is collapsed and the result is trimmed.
- `try_extract_prompt(line)` -- three-layer filter (see below).
- `extract_session_id(path)` -- takes the filename stem and validates it as a lowercase UUID.
- `is_valid_uuid(s)` -- byte-level check: exactly 36 chars, lowercase hex digits, hyphens at positions 8, 13, 18, 23.

### display.rs -- Output formatting

Three output modes, each taking `&[SessionInfo]` and a `now: DateTime<Utc>`:

- `format_default()` -- header ("Recent Claude Code sessions:"), aligned columns (index, relative time, project path, summary), footer ("Resume: ccsesh \<number>"). Prompts are quoted and truncated to 72 chars.
- `format_short()` -- compact single-line per session, no header/footer. Fixed 2-char index width, 3-char time width, prompts truncated to 52 chars without quotes.
- `format_json()` -- pretty-printed JSON array of `JsonSession` structs. No prompt truncation. Uses absolute paths with shell escaping in `resume_command`. Timestamps are ISO 8601 UTC with `Z` suffix.

Helper functions:
- `format_relative_time(duration)` -- `"<1m ago"`, `"Xm ago"`, `"Xh ago"`, etc. Negative durations clamped to `"<1m ago"`.
- `format_relative_time_short(duration)` -- same buckets without the `" ago"` suffix.
- `truncate_prompt(prompt, max)` -- truncates at last word boundary before `max - 3`, appends `"..."`. Hard-cuts if no space found.
- `display_summary(session)` -- priority cascade: prompt > slug > "(empty session)".

Colors use `owo_colors` with `if_supports_color(Stream::Stdout, ...)`, which respects both TTY detection and the `NO_COLOR` environment variable. Color scheme: cyan bold index, yellow time, green path, white prompt, dim+italic slug/empty fallback, dim header/footer.

### shell.rs -- Shell integration

- `print_shell_init(shell)` -- outputs the shell wrapper function for fish, bash, or zsh from embedded string constants. Returns `CcseshError::UnknownShell` for unrecognized shells.
- `print_exec_protocol(session)` -- validates the session UUID, then prints `__CCSESH_EXEC__` sentinel followed by `cd '<escaped_dir>' && claude --resume <uuid>`. The sentinel line is what the shell wrapper detects to switch from passthrough to eval mode.
- `print_resume_instructions(session)` -- human-readable fallback when `--shell-mode` is not set: `"To resume this session, run: cd ~/project && claude --resume <uuid>"`.
- `is_valid_uuid(s)` -- duplicate of the one in parse.rs; validates UUID format at the shell boundary as a security check before eval.

The wrapper functions (fish/bash/zsh) are stored as `const &str` templates. Each wrapper:
1. Calls `command ccsesh --shell-mode <shell> $argv` to get raw output.
2. Scans for the `__CCSESH_EXEC__` sentinel line.
3. If found, evals subsequent lines in the parent shell process.
4. Otherwise, prints output as-is with the original exit code.

## Key Design Decisions

### Why mtime instead of JSONL timestamps

Each JSONL line has a `timestamp` field, but ccsesh never parses it. Instead, it uses the file's filesystem mtime for both ranking and `last_active`. This is intentional: mtime is available from a `stat()` call during discovery (Phase 1) without opening the file, making it possible to rank hundreds of sessions by recency using only metadata. Parsing timestamps would require reading every file just to sort them.

### Why no regex crate (hand-written XML scanner)

The `strip_xml_tags()` function in parse.rs is a hand-written character scanner rather than a regex. This avoids adding the `regex` crate as a dependency for what is essentially a simple bracket-matching operation. The scanner has one important subtlety: if it encounters a newline before finding `>`, it treats the `<` as literal content. This handles cases like `"< 10 mins remaining\nPlease wrap up"` where `<` is a comparison operator, not a tag opener.

### Why three-layer prompt extraction

Claude Code JSONL files contain many `"type":"user"` lines that are not actual user prompts. The `try_extract_prompt()` function applies three filters in sequence:

1. **isMeta filter** -- Claude Code injects system context as user messages with `isMeta: true`. These are framework metadata, not user input.
2. **isCompactSummary filter** -- when context overflows, Claude Code injects a summary of the prior conversation as a user message with `isCompactSummary: true`. Displaying these as the session's "first prompt" would be misleading.
3. **Slash command detection** -- after XML stripping and whitespace collapse, if the text starts with `/` followed by an alphanumeric character, it is a slash command (`/clear`, `/add-dir`). These are tool invocations, not conversational prompts.

Only after passing all three filters is a message accepted as the session's display prompt.

### Why no rayon/parallelism

Session parsing is I/O-bound (reading small portions of files sequentially) and the typical workload is 5-20 files. The overhead of thread pool setup and synchronization would likely exceed any gains. The sequential loop in `main.rs` keeps the code simple and predictable.

### Why Option\<String> positional args instead of clap subcommands

The first positional argument (`command`) can be either `"init"`, a numeric index like `"3"`, or absent entirely. Clap subcommands would make `init` a proper subcommand but would not naturally handle the numeric-index case without a wrapper subcommand. Using `Option<String>` with manual dispatch in `run()` keeps the CLI surface minimal: `ccsesh`, `ccsesh 3`, `ccsesh init fish`.

### Why 50-line parse limit

Session JSONL files can grow to thousands of lines over a long conversation. The fields ccsesh needs (cwd, slug, first prompt) all appear near the top of the file. The `MAX_LINES = 50` constant caps how much is read per file, bounding I/O cost. The parser also terminates early if all three fields are found before reaching line 50.

## JSONL Parsing Rules

Each line in a Claude Code JSONL file is a JSON object with some subset of these fields:

| Field | Type | Meaning |
|-------|------|---------|
| `type` | `"user"` / `"assistant"` / `"summary"` | Message role |
| `cwd` | string | Working directory at time of message |
| `slug` | string | Three-word session name |
| `isMeta` | bool | Framework-injected system context |
| `isCompactSummary` | bool | Context-overflow summary injection |
| `sessionId` | string | UUID (not used; session ID comes from filename) |
| `timestamp` | string | ISO timestamp (not parsed; mtime used instead) |
| `message.content` | string or array | The message payload |

Content format has two shapes:
- **String**: `"content": "Hello world"` -- plain text.
- **Array**: `"content": [{"type": "image", ...}, {"type": "text", "text": "Describe this"}]` -- multimodal content. The parser extracts text from the first `{"type": "text"}` element.

Lines that fail JSON parsing are silently skipped. The parser reads up to 50 lines and takes the first occurrence of each field it needs.

## Error Handling Strategy

The crate uses a two-tier error strategy:

- **thiserror** (`CcseshError` in errors.rs) for domain-specific, user-facing errors with formatted messages. These are the errors users see: missing directories, out-of-range indices, unknown shells.
- **anyhow** at the top level (`run()` returns `anyhow::Result<()>`) for ad-hoc errors and for converting between error types via the `?` operator and `.into()`.

For individual session files, the strategy is **skip-on-failure**: if a single file cannot be read or parsed, it is silently skipped and the remaining sessions are still shown. Only if all discovered candidates fail to parse does the tool report `NoSessionsFound`. This prevents a single corrupted log file from breaking the entire listing.

## Testing Approach

### Unit Tests (116 tests, in-module)

Each module contains `#[cfg(test)] mod tests` with focused unit tests:

- **discover.rs** -- tests sorting order, limit clamping, limit=0 short-circuit, empty directories, non-JSONL filtering, nested directory exclusion, missing projects directory error, multi-project-dir merging, unreadable file handling.
- **parse.rs** -- tests for each sub-function (`strip_xml_tags`, `extract_text_from_content`, `is_valid_uuid`, `try_extract_prompt`) plus fixture-based `parse_session` tests. Fixtures are copied to temp files with UUID filenames since the parser validates filename format.
- **display.rs** -- tests for `format_relative_time` (all time buckets including negative clamping), `truncate_prompt` (within limit, word boundary, no-space hard cut), and each output format (empty sessions, column alignment, display priority cascade, JSON schema fields, nullable fields, no-truncation in JSON, absolute paths in resume commands, ISO 8601 timestamps).
- **shell.rs** -- UUID validation, template content assertions, exec protocol with valid/invalid UUIDs and paths with spaces/quotes.

### Fixtures (11 synthetic JSONL files in tests/fixtures/)

| Fixture | Tests |
|---------|-------|
| `normal.jsonl` | Standard session with cwd, slug, user/assistant messages |
| `meta_only.jsonl` | All user messages have `isMeta: true`; prompt should be `None` |
| `empty.jsonl` | Zero bytes; all fields should be `None`/empty |
| `xml_markup.jsonl` | XML tags in content; tests stripping and newline bail-out |
| `slash_command.jsonl` | Session starting with `/clear`, `/add-dir`; tests slash detection |
| `array_content.jsonl` | Content is an array with tool_result + text items |
| `image_paste.jsonl` | Content is an array with image + text items |
| `compact_summary.jsonl` | Contains `isCompactSummary: true` line to skip |
| `no_cwd.jsonl` | Missing cwd on all lines; project_dir defaults to empty |
| `summary_only.jsonl` | Only summary-type lines; no extractable prompt |
| `truncated.jsonl` | File cut off mid-line; tests graceful handling of incomplete data |

### Integration Tests (28 tests in tests/integration.rs)

Use `assert_cmd` to run the compiled binary as a subprocess with `assert_fs::TempDir` for isolation. Each test creates a temporary `$HOME` with synthetic `.claude/projects/` structure, copies fixtures in with deterministic UUIDs and controlled mtimes, then runs `ccsesh` with `HOME` overridden and `NO_COLOR=1` set.

Tests cover:
- Default/short/JSON output format correctness
- `--limit` behavior (restricts output count, affects resume index range)
- `--json` precedence over `--format`
- Resume with and without `--shell-mode`
- Out-of-range index errors
- `--shell-mode` without index error
- Unknown command errors
- `init` for all three shells plus error cases
- Empty/missing directory errors
- All-unparseable sessions treated as no sessions
- JSON schema field completeness
- Display priority (slug fallback, empty session fallback)
- Nullable JSON fields for meta-only and empty sessions
