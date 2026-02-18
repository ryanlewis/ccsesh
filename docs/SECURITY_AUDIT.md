# Security Audit — ccsesh

**Date:** 2026-02-18
**Scope:** Full source review of `src/`, install scripts, CI workflows, and dependencies.

---

## Executive Summary

ccsesh is a small, read-only CLI that parses local JSONL files and optionally emits shell commands for `eval`. The attack surface is narrow: untrusted input comes only from JSONL files in `~/.claude/projects/` and the `$HOME` environment variable. No network I/O, no privileged operations, no user-facing input beyond CLI args.

Overall the codebase is well-structured with good defensive practices (UUID validation before `eval`, single-quote shell escaping, parse caps, pinned CI actions). The findings below are ordered by severity.

---

## Findings

### 1. [Medium] Newline injection in Fish shell template via crafted `cwd`

**Location:** `src/shell.rs:26-31`, `src/types.rs:60-72`, Fish template at `src/shell.rs:65-84`

**Description:** The `print_exec_protocol` function emits a `cd '<path>' && claude --resume <uuid>` command that the shell wrapper `eval`s. The path is escaped via `shell_escape_single_quote`, which correctly handles single quotes but does **not** strip or escape newline characters.

In the **Fish** shell template, `command ccsesh ...` captures stdout and splits it into an array **by newline**. Each element is then `eval`'d independently:

```fish
for i in (seq (math $exec_idx + 1) (count $output))
    eval $output[$i]
end
```

If a JSONL file contains a `cwd` value with embedded newlines (e.g., `"/tmp/safe\nmalicious_command"`), the single `cd` command gets split across multiple array elements. The fragment after the newline is `eval`'d as a standalone command.

**Bash/zsh templates are not affected** — they `eval` the entire post-marker block as a single string, so newlines within single quotes remain part of the quoted path.

**Exploitability:** Low in practice. Requires write access to `~/.claude/projects/`, which implies local filesystem access. The JSONL files are written by Claude Code itself and are not user-editable in normal workflows. However, scenarios like shared filesystems, synced directories, or a compromised Claude Code process could introduce crafted files.

**Recommendation:** Reject or sanitize `cwd` values containing control characters (newlines, carriage returns, null bytes) in `parse_session`. A simple check:

```rust
if c.contains('\n') || c.contains('\r') || c.contains('\0') {
    continue; // skip malformed cwd
}
cwd = Some(c.clone());
```

Alternatively, `shell_escape_single_quote` could reject or strip these characters. The defense-in-depth approach would be to do both.

---

### 2. [Low] `truncate_prompt` panics on multi-byte UTF-8

**Location:** `src/display.rs:8-20`

**Description:** `truncate_prompt` uses byte-level indexing (`prompt.len()`, `prompt[..limit]`) but treats the limit as a character count conceptually. If the computed `limit` falls in the middle of a multi-byte UTF-8 character (e.g., emoji, CJK, accented characters), the slice `prompt[..limit]` will **panic** at runtime.

Example: A prompt with 68 ASCII bytes followed by a 2-byte UTF-8 character (70 bytes total). With `max_chars=72`, the early return passes. But if the prompt is 73 bytes with the last character being multi-byte, `limit = 69` could land mid-character.

**Impact:** Crash (not code execution). Since prompts come from local JSONL files written by Claude Code, this could be triggered by normal usage with non-ASCII prompts.

**Recommendation:** Use `char_indices()` or the `unicode-segmentation` crate for correct truncation:

```rust
pub fn truncate_prompt(prompt: &str, max_chars: usize) -> String {
    let char_count = prompt.chars().count();
    if char_count <= max_chars {
        return prompt.to_string();
    }
    let limit = max_chars.saturating_sub(3);
    // Find byte offset of the limit-th character
    let byte_limit = prompt.char_indices()
        .nth(limit)
        .map(|(i, _)| i)
        .unwrap_or(prompt.len());
    if let Some(pos) = prompt[..byte_limit].rfind(' ') {
        format!("{}...", &prompt[..pos])
    } else {
        format!("{}...", &prompt[..byte_limit])
    }
}
```

---

### 3. [Low] Symlink following in `discover_sessions`

**Location:** `src/discover.rs:31-78`

**Description:** The directory traversal uses `std::fs::read_dir` and `metadata()`, both of which follow symlinks. If an attacker places a symlink inside `~/.claude/projects/some-project/` pointing to a JSONL file outside the expected directory tree, `discover_sessions` will read it.

**Impact:** Minimal. The file is only parsed as JSONL — extracting `cwd`, `slug`, and prompt text. There's no arbitrary file disclosure. The attacker would need write access to `~/.claude/projects/`, which already implies local access.

**Recommendation:** Consider using `symlink_metadata()` instead of `metadata()` and skipping entries where the file type is a symlink:

```rust
let metadata = match file_path.symlink_metadata() {
    Ok(m) => m,
    Err(_) => continue,
};
if metadata.file_type().is_symlink() || !metadata.is_file() {
    continue;
}
```

This is a hardening measure, not a critical fix.

---

### 4. [Low] Install scripts proceed when checksum verification fails to download

**Location:** `install.sh:119-140`, `install.ps1:73-90`

**Description:** Both install scripts attempt SHA256 checksum verification of downloaded archives. If the `.sha256` file cannot be downloaded (e.g., network error, missing file), the scripts emit a warning but **continue with installation**. This means a tampered binary could be installed if an attacker can intercept the archive download but not the checksum download (or if the checksum file simply doesn't exist for a release).

**Recommendation:** Consider making checksum verification mandatory (fail-hard) rather than optional. If backwards compatibility is needed, add an environment variable opt-out:

```sh
if [ "${CCSESH_SKIP_CHECKSUM:-}" != "1" ]; then
    error "checksum verification failed — set CCSESH_SKIP_CHECKSUM=1 to bypass"
fi
```

---

### 5. [Low] Unescaped paths in human-readable resume instructions

**Location:** `src/shell.rs:35-41`

**Description:** `print_resume_instructions` outputs:
```
cd ~/my project && claude --resume <uuid>
```

The `project_dir_display` value (which uses `~` substitution) is not shell-escaped. If the path contains spaces, semicolons, or other shell metacharacters, a user who copy-pastes this command will get unexpected behavior.

**Recommendation:** Apply `shell_escape_single_quote` to the display path, or note in the output that paths with special characters need quoting.

---

### 6. [Informational] No `cargo audit` in CI

**Location:** `.github/workflows/ci.yml`

**Description:** The CI pipeline runs `cargo fmt`, `cargo clippy`, and `cargo test`, but does not run `cargo audit` to check for known vulnerabilities in dependencies. While the current dependency set is small and well-maintained, adding automated vulnerability checking is a low-effort improvement.

**Recommendation:** Add a `cargo audit` job:

```yaml
audit:
  name: Security Audit
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd
    - uses: rustsec/audit-check@69366f33c96575abad1ee0dba8212993eecbe998 # v2.0.0
```

---

### 7. [Informational] Duplicate `is_valid_uuid` implementations

**Location:** `src/parse.rs:215-242`, `src/shell.rs:43-63`

**Description:** Documented as intentional in CLAUDE.md (shell.rs copy is a security check before `eval`). The two implementations use slightly different algorithms (segment iteration vs. position matching) but are functionally equivalent. If one is updated and the other isn't, they could diverge.

**Recommendation:** Consider extracting to a shared `validation` module, or at minimum add a cross-reference comment and a test that verifies both implementations agree on a set of inputs.

---

## Positive Observations

These practices are worth calling out as good security hygiene:

- **UUID validation before `eval`**: The `is_valid_uuid` check in `shell.rs` is the critical security gate preventing arbitrary command injection via session IDs. It's strict (lowercase hex only, exact format) and correctly positioned.

- **Single-quote shell escaping**: `shell_escape_single_quote` in `types.rs` uses the standard `'\''` idiom, which is the correct way to escape for POSIX shells.

- **Pinned GitHub Actions**: All actions in CI and release workflows are pinned to full commit SHAs, not tags. This prevents supply chain attacks via tag mutation.

- **50-line parse cap**: `MAX_LINES = 50` in `parse.rs` bounds I/O and prevents DoS via large JSONL files.

- **Skip-on-failure parsing**: Malformed JSONL lines and unreadable files are silently skipped rather than crashing the process.

- **`--locked` in release builds**: Ensures the exact dependency versions from `Cargo.lock` are used.

- **Checksum verification in install scripts**: Both scripts verify SHA256 checksums when available.

- **Minimal dependency set**: Only 6 runtime dependencies, all well-maintained mainstream crates.
