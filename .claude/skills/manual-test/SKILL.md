---
name: manual-test
description: Run manual smoke tests of ccsesh against real session data
disable-model-invocation: true
---

# Manual Smoke Test

Build and run ccsesh against real `~/.claude/projects/` data to verify output visually.

Run each command and check the output looks correct:

```bash
# Build first
cargo build

# 1. Default format — should show header, indexed sessions, footer
cargo run

# 2. Short format — compact, no header/footer
cargo run -- --format short --limit 3

# 3. JSON format — valid JSON array with all schema fields
cargo run -- --json --limit 2

# 4. Resume (without shell wrapper) — should print "To resume this session, run:" instructions
cargo run -- 0

# 5. Shell init — should output a shell function
cargo run -- init fish
cargo run -- init bash
cargo run -- init zsh

# 6. Error cases
cargo run -- 999        # out of range
cargo run -- foobar     # unknown command
cargo run -- init       # missing shell arg
cargo run -- init nu    # unknown shell
```

**What to check:**
- Column alignment in default/short formats
- Relative times are reasonable (not showing negative or far-future)
- Project paths show `~/` prefix where applicable
- Prompts are truncated with `...` when long
- Sessions with no prompt fall back to slug or "(empty session)"
- JSON fields match schema documented in README (index, session_id, project_dir, project_dir_display, last_active, last_active_relative, first_prompt, slug, resume_command)
- Error messages go to stderr, exit code 1
