# ccsesh

Fast CLI tool to list and resume recent [Claude Code](https://docs.anthropic.com/en/docs/claude-code) sessions.

> **v0.1.0** — Rust, single binary, no runtime dependencies

## The Problem

Claude Code gives you `claude -c` (resume the most recent session) and `claude -r` (interactive picker), but no quick way to **list** recent sessions across all your projects. If you juggle multiple repos, finding that session from an hour ago means guessing or scrolling.

**ccsesh** fills this gap — it's fast enough for terminal MOTDs and simple enough to memorize.

## Installation

```sh
cargo install --path .
```

## Quick Start

```sh
# List your 5 most recent sessions
ccsesh

# Resume session #2
ccsesh 2

# Show 3 recent sessions in your shell MOTD
ccsesh --limit 3 --format short
```

## Usage

```
ccsesh [OPTIONS] [COMMAND] [SHELL]

Arguments:
  [COMMAND]  Session index to resume, or "init" subcommand
  [SHELL]    Shell type for init (fish, bash, zsh)

Options:
  -l, --limit <LIMIT>    Number of sessions to show [default: 5]
      --format <FORMAT>  Output format: default, short [default: default]
      --json             Output as JSON array
  -h, --help             Print help
  -V, --version          Print version
```

### Commands

| Command | Description |
|---------|-------------|
| `ccsesh` | List recent sessions |
| `ccsesh <N>` | Resume session at index N |
| `ccsesh init <shell>` | Print shell wrapper function (fish, bash, zsh) |

### Hidden Flags

| Flag | Description |
|------|-------------|
| `--shell-mode <shell>` | Used internally by the shell wrapper to enable the exec protocol. Not shown in `--help`. |

### Format Precedence

`--json` takes precedence over `--format` — if both are provided, `--format` is silently ignored.

## Output Examples

### Default format

```
$ ccsesh
Recent Claude Code sessions:

  0  <1m ago   ~/dev/myapp     "Add user authentication with JWT tokens and refresh..."
  1   3m ago   ~/dev/myapp     "Fix pagination bug in the /users endpoint that returns..."
  2  15m ago   ~/dev/api       "Refactor database connection pooling to use deadpool..."
  3   1h ago   ~/dotfiles      "Set up neovim LSP config for Rust and TypeScript..."
  4   2h ago   ~/dev/frontend  "Implement dark mode toggle with system preference..."

Resume: ccsesh <number>
```

### Short format

```
$ ccsesh --format short
 0 <1m  ~/dev/myapp     Add user authentication with JWT tokens...
 1  3m  ~/dev/myapp     Fix pagination bug in the /users endpoint...
 2 15m  ~/dev/api       Refactor database connection pooling to use...
 3  1h  ~/dotfiles      Set up neovim LSP config for Rust and...
 4  2h  ~/dev/frontend  Implement dark mode toggle with system...
```

### JSON format

```
$ ccsesh --json
```

```json
[
  {
    "index": 0,
    "session_id": "3ab5f3ce-483e-4f9e-8772-cb488b79f3cc",
    "project_dir": "/home/user/dev/myapp",
    "project_dir_display": "~/dev/myapp",
    "last_active": "2026-02-18T00:59:24Z",
    "last_active_relative": "<1m ago",
    "first_prompt": "Add user authentication with JWT tokens and refresh token rotation",
    "slug": "flickering-jumping-raven",
    "resume_command": "cd '/home/user/dev/myapp' && claude --resume 3ab5f3ce-483e-4f9e-8772-cb488b79f3cc"
  }
]
```

JSON output preserves full prompt text (no truncation). Fields `first_prompt` and `slug` are nullable.

### Resume a session

Without the shell wrapper installed:

```
$ ccsesh 0
To resume this session, run:
  cd ~/dev/myapp && claude --resume 3ab5f3ce-483e-4f9e-8772-cb488b79f3cc
```

With the shell wrapper, `ccsesh 0` resumes directly in your current shell.

## Shell Integration

The shell wrapper lets `ccsesh <N>` resume sessions directly instead of printing a command to copy-paste. One-liner setup for each shell:

**Fish:**
```sh
ccsesh init fish | source
```

**Bash:**
```sh
eval "$(ccsesh init bash)"
```

**Zsh:**
```sh
eval "$(ccsesh init zsh)"
```

To persist across sessions, add the line to your shell's config file (`~/.config/fish/conf.d/ccsesh.fish`, `~/.bashrc`, or `~/.zshrc`).

See [docs/shell-integration.md](docs/shell-integration.md) for the full guide, including how the `__CCSESH_EXEC__` protocol works under the hood.

## MOTD Recipe

Show your recent Claude Code sessions every time you open a terminal. Add this to `~/.config/fish/conf.d/ccsesh.fish`:

```fish
if status is-interactive
    and command -q ccsesh
    set -l out (command ccsesh --limit 3 --format short 2>/dev/null)
    if test -n "$out"
        echo $out
    end
end
```

## How It Works

ccsesh operates in two phases: **discover** and **parse**. First, it enumerates all `.jsonl` session files under `~/.claude/projects/`, stats each for mtime, sorts by most recent, and keeps the top N. Then it reads up to 50 lines from each selected file to extract the session ID (from the filename), working directory, slug, and first user prompt — skipping meta messages, compact summaries, and slash commands. Sequential I/O is fast enough that no parallelism (rayon, etc.) is needed; the whole operation typically completes in single-digit milliseconds. Colors are applied only when stdout is a TTY and `NO_COLOR` is not set. See [Performance](#performance) for benchmark results.

## Performance

ccsesh is designed for speed — fast enough to include in your terminal MOTD without noticeable delay.

**Typical latency:**
- **Discovery (50 sessions):** ~82µs (stat 50 files, sort by mtime)
- **Parse per session:** ~3.6µs (read top 50 lines, extract cwd/slug/prompt)
- **End-to-end (5 sessions):** <500µs total
- **Display formatting:** 2-3µs (default), 5µs (JSON)

**Benchmark environment:**
- Linux container (6.12.67 kernel)
- NVMe SSD storage
- Rust 1.84.0 (release build with optimizations)
- Test data: synthetic JSONL fixtures

Run your own benchmarks: `cargo bench`

**Why so fast?**
1. **Two-phase pipeline** — stat-only discovery, then selective parsing
2. **Bounded I/O** — reads max 50 lines per file (cwd/slug/prompt are near top)
3. **No regex** — hand-written XML stripper, simple bracket matching
4. **Sequential I/O** — no rayon/threading overhead (small files = sequential wins)
5. **Minimal allocations** — reuses buffers where possible

See `benches/internals.rs` for detailed benchmark code.

## Contributing

```sh
# Run all tests (144: 116 unit + 28 integration)
cargo test
```

### Project Structure

```
src/
  main.rs       — CLI entry point, clap parsing, command dispatch
  types.rs      — Shared structs and utilities
  errors.rs     — Error types (thiserror)
  discover.rs   — Session file discovery (stat + sort by mtime)
  parse.rs      — JSONL parsing and prompt extraction
  display.rs    — Output formatting (default, short, JSON)
  shell.rs      — Shell wrapper generation and exec protocol

tests/
  integration.rs  — End-to-end CLI tests (assert_cmd)
  fixtures/       — Synthetic JSONL test files
```

See [docs/internals.md](docs/internals.md) for architecture details, the three-layer prompt extraction pipeline, and the data flow diagram.
