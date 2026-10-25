# Release readiness

Use this checklist for every tagged release. A successful build is necessary,
but it is not sufficient evidence that the release is safe to publish.

## Hard gates

- [ ] The release commit and annotated tag are reviewed, pushed, and the
      worktree is clean.
- [ ] `[Unreleased]` contains user-facing fixes, compatibility changes,
      migrations, and security notes appropriate for public disclosure.
- [ ] `make ci-check` passes after the final edit. Platform CI passes on Linux,
      Windows, and macOS, including the pinned LuaJIT runtime oracle on Linux.
- [ ] `npm audit --prefix editors/vscode --audit-level=high` and
      `make rust-audit` pass against the committed lockfiles.
- [ ] The trusted release workflow succeeds before any registry credential is
      used. Every native binary passes its version smoke, every checksum
      verifies, and every release artifact has a build-provenance attestation.
- [ ] A maintainer downloads one archive and one VSIX from the published GitHub
      release, verifies the checksum and attestation, and runs the packaged
      binary rather than a workspace build.
- [ ] Visual Studio Marketplace and Open VSX show the same version and platform
      packages as the GitHub release.

## Editor documentation evidence

- [ ] For editor UI changes, `make extension-screenshots` completes against the
      candidate and every captured image receives a visual review. Otherwise,
      review the existing gallery and record why recapture was unnecessary.
- [ ] `make extension-screenshots-check` passes; README captions match the panels.
      `editors/vscode/docs/screenshots/capture.json` records the capture versions,
      timestamp, bundle hash, and image hashes. Explain any candidate mismatch.
- [ ] Missing native dependencies or graphical access are recorded as pending;
      a headless artifact check is not a substitute for a new native capture.

## Minor-release qualification

For a minor release, also record evidence for these checks in the release issue
or maintainer notes:

- [ ] Build, check, format, watch/dev, packaging, and editor workflows were
      exercised on at least one independent project not maintained as a Tua
      repository fixture. Record only sanitized project characteristics and
      outcomes; do not copy private source.
- [ ] Install the previous release, open the pilot project, upgrade to the
      candidate, and confirm configuration and generated output migrations.
- [ ] Reinstall the previous VSIX and standalone binary to confirm the project
      remains usable, or document why a generated-output migration is
      intentionally one-way.
- [ ] Review representative compiler, completion, editor-contention, and memory
      reports for regressions rather than relying only on the smoke budget.

Experimental features that are disabled by default do not block a release, but
their defaults must not change without independent project evidence and an
explicit release note.

## Security releases

Backport a security fix to the supported release line when unrelated features
on `main` would violate patch-version expectations. Prepare the changelog,
private advisory, patched artifacts, and verification instructions before
public disclosure. Publish the advisory only after users can obtain the fixed
version.

Checksums and GitHub attestations do not replace platform code signing. Keep
signing in the roadmap until maintainers have sustainable identities, protected
keys, and a tested renewal and revocation process.
