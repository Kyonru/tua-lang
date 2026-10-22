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
- [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=SolenodonteLabs.tua)
  and [Open VSX](https://open-vsx.org/extension/SolenodonteLabs/tua) publish the same
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

!!! warning "Verify release artifacts before installing them"
    Release assets are checksummed rather than code-signed today. Compare the
    downloaded archive or VSIX against its entry in `SHA256SUMS` before running
    or installing it.

On Linux:

```bash
sha256sum --check SHA256SUMS
```

On macOS:

```bash
shasum -a 256 tua-0.3.0-darwin-arm64.vsix
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

1. Add release notes under `[Unreleased]`, including compatibility and
   migration notes when needed, commit them, and leave the worktree clean.
2. Run the required extension-host suite.
3. Run `make release-version VERSION=<version>`. The guarded script updates
   Rust, VS Code, lockfile, changelog, and release-documentation versions; runs
   `make ci-check`; creates a `release: v<version>` commit; and adds an
   annotated `v<version>` tag. It refuses empty release notes, dirty worktrees,
   non-increasing versions, and existing tags.
4. Inspect the commit and tag, then push both with
   `git push origin HEAD v<version>`. The script never pushes automatically.
5. The release workflow validates the tag and changelog, invokes the complete
   trusted CI workflow, builds all platform pairs only after those gates pass,
   verifies checksums, and publishes the GitHub release.
6. After the release workflow completes successfully, the extension publish
   workflow downloads its published VSIX assets and pushes them to Visual
   Studio Marketplace and Open VSX. Failed releases never start publishing.

Manual dispatch of the release workflow is accepted only when it runs from the
exact matching tag. The extension workflow can also be dispatched manually with
an existing release tag. This prevents an untagged build from publishing
versioned artifacts while preserving a recovery path for extension registries.

See [CHANGELOG.md](https://github.com/Kyonru/tua-lang/blob/main/CHANGELOG.md)
for release-specific additions and migration notes.
