---
icon: lucide/package-check
---

# Releases And Compatibility

Tua coordinates the Rust crates, `tua` CLI, language server, and VS Code
extension under one version. The authoritative value is
`[workspace.package].version` in the root `Cargo.toml`; generated package
metadata is checked by `make version-drift-check`.

## Published Channels

- [GitHub Releases](https://github.com/Kyonru/tua-lang/releases) ship the
  standalone `tua` archives, platform VSIX packages, per-file checksums, and
  the combined `SHA256SUMS` manifest.
- [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=tua-lang.tua)
  and [Open VSX](https://open-vsx.org/extension/tua-lang/tua) publish the same
  platform VSIX packages after the matching GitHub release is published.
- [GitHub Pages](https://kyonru.github.io/tua-lang/) serves the public
  documentation site from the repository's `main` branch.

## Supported Release Artifacts

Tagged releases build these server and VSIX pairs:

| Platform | Rust target | VSIX target |
| --- | --- | --- |
| Linux x64 | `x86_64-unknown-linux-gnu` | `linux-x64` |
| Windows x64 | `x86_64-pc-windows-msvc` | `win32-x64` |
| macOS Intel | `x86_64-apple-darwin` | `darwin-x64` |
| macOS Apple silicon | `aarch64-apple-darwin` | `darwin-arm64` |

Every platform VSIX contains the matching `tua` server and uses it by default.
Standalone server archives include the executable, README, license, and
changelog. Tua does not download or execute an unverified server after
extension installation.

Release assets are checksummed rather than code-signed today. Every archive and
VSIX has an individual `.sha256` file, and the release includes one
`SHA256SUMS` manifest verified by CI before publication.

## Verify A Download

On Linux:

```bash
sha256sum --check SHA256SUMS
```

On macOS:

```bash
shasum -a 256 tua-0.1.0-darwin-arm64.vsix
```

Compare the printed value with the matching line in `SHA256SUMS`. On Windows,
use `Get-FileHash <file> -Algorithm SHA256` and compare the result.

## Compatibility Policy

Before 1.0, Tua uses semantic versions with these expectations:

- patch releases fix behavior without intentionally changing valid source;
- minor releases may evolve syntax, type precision, configuration, or editor
  behavior when the changelog includes migration notes;
- the CLI, server, and extension from one release are tested and distributed as
  a matching set;
- newer extensions may launch a user-selected older server, but only matching
  release pairs are guaranteed and tested;
- generated Lua continues to target Lua 5.1-style Lua, LuaJIT, and LOVE 11.x
  unless a release note explicitly announces a target change.

The VS Code extension requires VS Code 1.94 or newer. That baseline provides
the native inlay-hint maximum-length setting used by Tua's compact-hint toggle.

## Release Process

1. Update `[Unreleased]` and add a dated version section in `CHANGELOG.md` with
   compatibility and migration notes.
2. Change the root workspace version and run `make sync-version`.
3. Run `make ci-check` and the required extension-host suite.
4. Tag the exact commit as `v<version>` and push the tag.
5. The release workflow validates the tag and changelog, builds all platform
   pairs, verifies checksums, and publishes the GitHub release.
6. The extension publish workflow downloads the published release VSIX assets
   and pushes them to Visual Studio Marketplace and Open VSX.

Manual workflow dispatch is accepted only when it runs from the exact matching
tag. This prevents an untagged branch from publishing versioned artifacts.

See [CHANGELOG.md](https://github.com/Kyonru/tua-lang/blob/main/CHANGELOG.md)
for release-specific additions and migration notes.
