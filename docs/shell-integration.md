# Shell Integration

When you resume a Claude Code session, you typically need to change to
the project directory first. But a child process (like `ccsesh`) cannot
change the working directory of your parent shell. Shell integration
solves this by installing a thin wrapper function that intercepts resume
commands so the `cd` happens in your current shell before launching
Claude Code.

## Setup

Add the appropriate line to your shell configuration file, then restart
your shell or source the file.

### Fish

Add to `~/.config/fish/conf.d/ccsesh.fish`:

```fish
ccsesh init fish | source
```

### Bash

Add to `~/.bashrc`:

```bash
eval "$(ccsesh init bash)"
```

### Zsh

Add to `~/.zshrc`:

```zsh
eval "$(ccsesh init zsh)"
```

### Quick Setup

One-liners you can paste directly into your terminal:

```sh
# Fish
mkdir -p ~/.config/fish/conf.d && echo 'ccsesh init fish | source' >> ~/.config/fish/conf.d/ccsesh.fish

# Bash
echo 'eval "$(ccsesh init bash)"' >> ~/.bashrc

# Zsh
echo 'eval "$(ccsesh init zsh)"' >> ~/.zshrc
```

Restart your shell afterwards, or source the relevant config file.

## How the Wrapper Works

The shell function intercepts resume commands (`ccsesh <number>`) so it
can change your working directory to the session's project before
launching Claude Code. When listing sessions (no arguments), it passes
output through unchanged.

Without the wrapper, `ccsesh 0` prints instructions you have to
copy-paste manually:

```
To resume this session, run:
  cd ~/myproject && claude --resume abc12345-...
```

With the wrapper loaded, `ccsesh 0` changes to the project directory and
launches Claude Code in one step.

## MOTD / Startup Integration

You can show recent sessions automatically when you open a new terminal.
Use `--limit` and `--format short` for a compact summary.

### Fish

Create `~/.config/fish/conf.d/ccsesh-motd.fish`:

```fish
if status is-interactive
    ccsesh --limit 3 --format short
end
```

### Bash

Add to `~/.bashrc`:

```bash
ccsesh --limit 3 --format short
```

### Zsh

Add to `~/.zshrc`:

```zsh
ccsesh --limit 3 --format short
```

## Troubleshooting

### "command not found" after install

The `ccsesh` binary is installed to `~/.cargo/bin/` by default. Make
sure this directory is in your `PATH`:

- **Fish**: `fish_add_path ~/.cargo/bin`
- **Bash/Zsh**: Add `export PATH="$HOME/.cargo/bin:$PATH"` to your
  shell config file

### Wrapper not loaded

Verify the shell config file exists and contains the `ccsesh init` line:

- **Fish**: Check `~/.config/fish/conf.d/ccsesh.fish`
- **Bash**: Check `~/.bashrc`
- **Zsh**: Check `~/.zshrc`

After editing, restart your shell or source the config file (e.g.,
`source ~/.bashrc`).

You can confirm the wrapper is active by running `type ccsesh`. It
should report a function, not just the binary path.

### Sessions not showing

ccsesh reads session data from `~/.claude/projects/`. If no sessions
appear:

- Confirm Claude Code is installed and has been used at least once
- Check that `~/.claude/projects/` exists and contains `.jsonl` files
- Try increasing the limit: `ccsesh --limit 20`

### Colors not showing

ccsesh uses colors only when stdout is a terminal (TTY). Colors are
suppressed when:

- Output is piped or redirected
- The `NO_COLOR` environment variable is set
- Your terminal does not support color

To check: run `ccsesh` directly in your terminal (not piped through
another command). If you have `NO_COLOR` set, unset it:

```bash
unset NO_COLOR
```

## Uninstall

1. Remove the shell integration lines from your config files:
   - **Fish**: Delete `~/.config/fish/conf.d/ccsesh.fish`
   - **Bash**: Remove the `eval "$(ccsesh init bash)"` line from `~/.bashrc`
   - **Zsh**: Remove the `eval "$(ccsesh init zsh)"` line from `~/.zshrc`

2. If you added MOTD lines, remove those as well.

3. Uninstall the binary:

```
cargo uninstall ccsesh
```
