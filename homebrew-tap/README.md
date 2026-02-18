# homebrew-ccsesh

Homebrew tap for [ccsesh](https://github.com/ryanlewis/ccsesh) -- list and resume recent Claude Code sessions.

## Install

```sh
brew install ryanlewis/ccsesh/ccsesh
```

Or add the tap first:

```sh
brew tap ryanlewis/ccsesh
brew install ccsesh
```

## Updating

```sh
brew update && brew upgrade ccsesh
```

## How the formula is updated

The formula is updated automatically on each release. The CI release workflow builds platform binaries, computes SHA256 checksums, and pushes an updated `Formula/ccsesh.rb` to this repository. See the main [ccsesh](https://github.com/ryanlewis/ccsesh) repository for details.
