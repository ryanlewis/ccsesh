---
name: release
description: Bump version in Cargo.toml, commit, tag, and push to trigger the release pipeline
disable-model-invocation: true
---

# Release

Create a new release of ccsesh. The CI release workflow (`.github/workflows/release.yml`) handles building, packaging, and publishing automatically when a `v*` tag is pushed.

## Arguments

The user should provide the new version number (e.g. `0.2.0`). If not provided, ask for it.

## Steps

1. **Confirm current version**: read `version` from `Cargo.toml` and show it to the user
2. **Bump version**: update the `version` field in `Cargo.toml` to the new version
3. **Update lockfile**: run `cargo build` to update `Cargo.lock`
4. **Run checks**: run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` — abort if anything fails
5. **Commit**: `chore: Bump version to {version}`
6. **Tag**: `git tag v{version}`
7. **Push**: `git push && git push --tags`

After pushing, remind the user:
- CI will build binaries, create a GitHub Release, and update the Homebrew tap
- Winget manifest needs a manual update (see `docs/distribution.md`)
