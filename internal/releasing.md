# Preparing A Tua Release

This is the maintainer workflow. For downloads, upgrades, and compatibility,
see [Releases And Compatibility](../docs/releases.md).

## Procedure

1. Add detailed release notes under `[Unreleased]`. Before preparing a release,
   lead them with `### Release summary`: a short overview, `#### Highlights`
   with three to six user-facing bullets, and `#### Upgrade notes` when needed.
   Aim for 150–300 words and include material compatibility or security changes.
   The next level-three heading ends the summary. The GitHub release generator
   publishes only this summary, promotes its subheadings, and adds a link to the
   full tagged changelog and download/checksum guidance. Older entries without
   a summary retain full-note extraction. Commit the notes and leave the
   worktree clean.
2. Refresh the editor README screenshots when the release changes editor UI,
   and review their captions and visual quality. Run `make extension-screenshots`
   in a graphical session with a staged compiler and local VS Code/VSCodium;
   `make extension-screenshots-check` verifies existing artifacts without a GUI.
   See the repository's `.codex/skills/tua-release-prep/SKILL.md` for preparation and
   provenance checks. Run the required extension-host suite.
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
