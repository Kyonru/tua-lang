---
name: tua-release-prep
description: Prepare a Tua release candidate with current changelog summaries, editor README screenshots, and verification evidence. Use for release preparation or refreshing release-facing editor documentation; preparation alone does not tag, push, or publish.
---

# Tua Release Preparation

Work from the Tua repository root. Read `docs/releases.md` and
`internal/release-readiness.md` for the current release process and gates.
Preserve unrelated changes; do not infer publishing authorization from a request
to prepare artifacts.

## Notes and editor gallery

- Follow `skills/tua-doc-updater/SKILL.md` for the changelog summary and affected
  documentation. Summarize implemented behavior, including compatibility changes.
- When preparing a release containing editor UI changes, regenerate the README
  gallery after the final UI edit. For other releases, inspect existing images
  and run `make extension-screenshots-check`; record why recapture was unnecessary.
- Ensure extension dependencies and a candidate compiler are staged. Use the
  existing `make extension-deps extension-server` targets when needed. Do not
  silently use a stale bundled compiler for release evidence.
- Run `make extension-screenshots` in a graphical local session. Set
  `VSCODE_EXECUTABLE_PATH` to the local VS Code/VSCodium executable when needed.
  It builds the production extension, captures all five actual feature views in
  a disposable profile/project, and validates the completed gallery. It neither
  installs into the normal profile nor publishes anything.
- Read `editors/vscode/docs/screenshots/README.md` for capture details. Keep the
  behavior in repository scripts, not copied into this skill. To review without
  replacing tracked images, set `TUA_CAPTURE_OUTPUT` to a temporary directory.
- Inspect every resulting PNG with an image viewer: populated graphs, complete
  controls, readable text, expanded runtime values, no errors or personal data.
  Automated hashes and dimensions do not establish visual quality. Keep README
  captions and alt text accurate; do not describe static graphs as live graphs.
- Check `capture.json` for capture time, extension/compiler versions, bundle hash,
  and complete image hashes. The command records versions rather than enforcing
  equality: explain any mismatch, and restage/recapture when it affects the release
  candidate. A subsequent version-only release bump can retain reviewed images
  when the UI is unchanged; record that provenance rather than rewriting it.
- If a panel fails, fix and verify it before using its screenshot. If the host
  lacks a GUI, LÖVE, or a local editor, preserve the last complete gallery and
  report capture as pending. Do not publish partial or fabricated replacements.

## Verification and handoff

Use `skills/tua-verification/SKILL.md` for the code and release gates. Screenshot
capture supplements the extension-host suite and `make ci-check`; it does not
replace either. Run `make extension-screenshots-check docs-build diff-check`
after documentation edits. Report actual passes, failures, and unrun gates.

Commit independent fixes separately from gallery/docs changes, using
`--no-gpg-sign` and no coauthor trailer for this repository. Leave version/tag
creation to the user's requested release scope and the existing guarded
`make release-version VERSION=...` workflow. Do not push or publish just because
release-preparation checks passed.
