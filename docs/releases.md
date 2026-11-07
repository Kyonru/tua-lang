---
icon: lucide/package-check
---

# Releases And Compatibility

Tua coordinates the Rust crates, `tua` CLI, language server, and VS Code
extension under one version. Use matching extension and server versions,
especially when selecting a custom server binary.

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

Release assets are checksummed and receive GitHub build-provenance attestations,
but are not platform code-signed today. Every archive and VSIX has an individual
`.sha256` file, and the release includes one `SHA256SUMS` manifest verified by
CI before publication.

## Verify A Download

!!! warning "Verify release artifacts before installing them"
    Release assets are checksummed and attested, but are not platform
    code-signed today. Compare the downloaded archive or VSIX against its entry
    in `SHA256SUMS` before running or installing it.

On Linux:

```bash
sha256sum --check SHA256SUMS
```

On macOS:

```bash
shasum -a 256 tua-0.6.0-darwin-arm64.vsix
```

Compare the printed value with the matching line in `SHA256SUMS`. On Windows,
use `Get-FileHash <file> -Algorithm SHA256` and compare the result.

With the GitHub CLI installed, verify that an artifact was produced by Tua's
release workflow from this repository:

```bash
gh attestation verify tua-0.6.0-aarch64-apple-darwin.tar.gz \
  --repo Kyonru/tua-lang
```

Provenance complements the checksum manifest; it is not a substitute for
platform signing or for reviewing which tagged source was released.

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

## Upgrade An Existing Project

1. Read the target release's changelog for syntax, configuration, or inference
   changes. Keep a version-control checkpoint before upgrading.
2. Update the extension and standalone CLI together when you use both. If
   `tua.server.path` selects a custom server, update that binary or return to
   the bundled server with **Tua: Select Server**.
3. Check the editor's language status and `tua --version` to confirm which
   binaries are active.
4. Run `tua check`, `tua fmt --check`, and `tua build` in your project. Review
   diagnostics before applying formatting or changing source.
5. Start the game and test the behavior you depend on. Static checks do not
   replace gameplay or platform testing.

To try or return to a particular editor version, install its platform VSIX
through **Extensions: Install from VSIX…**. Keep the CLI/server pair matched;
older versions may not understand source or configuration added by a newer one.

The documentation website follows `main` and can describe unreleased changes.
When a guide and your installed release differ, consult the changelog and the
`docs/` files at that release's Git tag.

## Maintainer Workflow {#release-process}

Release preparation, tagging, and publication are maintainer operations that
follow the trusted release workflow. See
[CHANGELOG.md](https://github.com/Kyonru/tua-lang/blob/main/CHANGELOG.md) for
release-specific additions and migration notes.
