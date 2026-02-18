# Distribution Guide

Step-by-step instructions for setting up and maintaining ccsesh's distribution channels.

## Overview

| Channel | Platforms | Install Command |
|---------|-----------|-----------------|
| Homebrew | macOS, Linux | `brew install ryanlewis/ccsesh/ccsesh` |
| install.sh | macOS, Linux | `curl -fsSL https://raw.githubusercontent.com/ryanlewis/ccsesh/main/install.sh \| sh` |
| install.ps1 | Windows | `irm https://raw.githubusercontent.com/ryanlewis/ccsesh/main/install.ps1 \| iex` |
| winget | Windows | `winget install ryanlewis.ccsesh` |
| Cargo | All | `cargo install --path .` |

## Release Workflow

1. Bump version in `Cargo.toml`
2. `cargo build` to update `Cargo.lock`
3. Commit: `git commit -am "Bump version to 0.2.0"`
4. Tag: `git tag v0.2.0`
5. Push: `git push && git push --tags`
6. CI automatically:
   - Builds binaries for all 4 targets
   - Creates archives with binary + LICENSE + README.md
   - Generates SHA256 checksums
   - Creates a GitHub Release with all assets
   - Updates the Homebrew tap formula
7. Manually: update winget manifest (see below)

## One-Time Setup

### 1. GitHub Repository Secrets

Add these secrets at `github.com/ryanlewis/ccsesh/settings/secrets/actions`:

| Secret | Purpose | How to Get |
|--------|---------|------------|
| `HOMEBREW_TAP_TOKEN` | Push formula updates to the tap repo | Create a fine-grained PAT at github.com/settings/tokens with `Contents: Read and write` permission scoped to the `homebrew-ccsesh` repo |

The `GITHUB_TOKEN` is automatically available in workflows and handles release creation.

### 2. Homebrew Tap Repository

Create a public repo named `homebrew-ccsesh` under your GitHub account:

```sh
# Create the repo on GitHub (via web UI or gh CLI)
gh repo create ryanlewis/homebrew-ccsesh --public --description "Homebrew tap for ccsesh"

# Clone and set up initial structure
git clone git@github.com:ryanlewis/homebrew-ccsesh.git
cd homebrew-ccsesh
mkdir -p Formula

# Copy the formula template (the CI will overwrite this on first release)
cp /path/to/ccsesh/homebrew-tap/Formula/ccsesh.rb Formula/
cp /path/to/ccsesh/homebrew-tap/README.md .

git add .
git commit -m "Initial tap setup"
git push
```

After the first release, the CI `publish-homebrew` job will automatically update the formula with correct SHA256 checksums.

### 3. Winget Submission (First Time)

The initial submission requires a manual PR to `microsoft/winget-pkgs`:

```sh
# Fork microsoft/winget-pkgs on GitHub, then:
git clone git@github.com:ryanlewis/winget-pkgs.git
cd winget-pkgs

# Create the directory structure
mkdir -p manifests/r/ryanlewis/ccsesh/0.1.0/

# Copy manifest files from this repo
cp /path/to/ccsesh/winget/manifests/r/ryanlewis/ccsesh/0.1.0/*.yaml \
   manifests/r/ryanlewis/ccsesh/0.1.0/
```

Before submitting, replace the SHA256 placeholder in the installer manifest:

```sh
# Download the release asset and compute its hash
curl -fsSL https://github.com/ryanlewis/ccsesh/releases/download/v0.1.0/ccsesh-x86_64-pc-windows-msvc.zip \
  | sha256sum | awk '{print $1}'

# Replace PLACEHOLDER_SHA256_WINDOWS_X64 with the actual hash
```

Then submit:

```sh
git checkout -b ccsesh-0.1.0
git add manifests/
git commit -m "New package: ryanlewis.ccsesh version 0.1.0"
git push origin ccsesh-0.1.0
# Open PR to microsoft/winget-pkgs via GitHub web UI
```

Automated bots will validate the manifest format and URL accessibility. A Microsoft reviewer typically approves within 1-3 days.

### 4. Winget Updates (Subsequent Releases)

For future releases, use `wingetcreate` to automate the update:

```powershell
# Install wingetcreate
winget install wingetcreate

# Update manifest and submit PR automatically
wingetcreate update ryanlewis.ccsesh `
  --urls https://github.com/ryanlewis/ccsesh/releases/download/v0.2.0/ccsesh-x86_64-pc-windows-msvc.zip `
  --version 0.2.0 `
  --submit
```

This can also be added as a CI step using a GitHub PAT with `public_repo` scope.

## Target Matrix

| Target Triple | OS | Arch | Build Method | Archive |
|---|---|---|---|---|
| `aarch64-apple-darwin` | macOS | ARM64 | Native (macos-latest) | `.tar.gz` |
| `x86_64-unknown-linux-gnu` | Linux | x86_64 | Native (ubuntu-latest) | `.tar.gz` |
| `aarch64-unknown-linux-gnu` | Linux | ARM64 | cross-rs (ubuntu-latest) | `.tar.gz` |
| `x86_64-pc-windows-msvc` | Windows | x86_64 | Native (windows-latest) | `.zip` |

## Release Asset Naming

Each release at `github.com/ryanlewis/ccsesh/releases/tag/v{version}` contains:

```
ccsesh-aarch64-apple-darwin.tar.gz
ccsesh-aarch64-apple-darwin.tar.gz.sha256
ccsesh-x86_64-unknown-linux-gnu.tar.gz
ccsesh-x86_64-unknown-linux-gnu.tar.gz.sha256
ccsesh-aarch64-unknown-linux-gnu.tar.gz
ccsesh-aarch64-unknown-linux-gnu.tar.gz.sha256
ccsesh-x86_64-pc-windows-msvc.zip
ccsesh-x86_64-pc-windows-msvc.zip.sha256
checksums.txt
```

Each `.tar.gz` / `.zip` contains: `ccsesh` (or `ccsesh.exe`), `LICENSE`, `README.md`.

Each `.sha256` file contains: `{hash}  {filename}` (two-space separated, matching sha256sum output).

`checksums.txt` is a concatenation of all `.sha256` files.

## CI Workflows

### ci.yml

Runs on push to main and PRs:
- **fmt**: `cargo fmt --check`
- **clippy**: `cargo clippy -- -D warnings`
- **test**: `cargo test` on Linux, macOS, Windows

### release.yml

Triggered by `v*` tags:
- **build**: Compiles release binaries for all 4 targets, packages archives with checksums
- **release**: Creates GitHub Release with all assets
- **publish-homebrew**: Updates the Homebrew tap formula with new version and checksums

## Install Script Configuration

Both install scripts support environment variable overrides:

### install.sh

| Variable | Default | Description |
|----------|---------|-------------|
| `VERSION` | latest | Version to install (e.g. `v0.1.0` or `0.1.0`) |
| `CCSESH_INSTALL_DIR` | `~/.local/bin` | Installation directory |

### install.ps1

| Variable | Default | Description |
|----------|---------|-------------|
| `VERSION` | latest | Version to install (e.g. `v0.1.0` or `0.1.0`) |
| `CCSESH_INSTALL_DIR` | `~\.ccsesh\bin` | Installation directory |

## Troubleshooting

### Homebrew formula not updating after release

Check that:
1. The `HOMEBREW_TAP_TOKEN` secret is set and the PAT has `Contents: Read and write` on `homebrew-ccsesh`
2. The `homebrew-ccsesh` repo exists and has a `Formula/` directory
3. The `publish-homebrew` job ran successfully (check Actions tab)

### Winget submission rejected

Common reasons:
- SHA256 mismatch: recompute from the actual release asset
- URL not accessible: ensure the GitHub Release is public
- Manifest validation errors: run `winget validate` locally
