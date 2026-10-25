---
name: tua-verification
description: Choose and run the right checks for Tua changes. Use before finalizing code, docs, examples, VS Code extension, LSP, CLI, parser, checker, emitter, LOVE, asset, or roadmap changes, especially when deciding between focused tests and full workspace verification.
---

# Tua Verification

Verify according to risk and blast radius.

## Verification Ladder

1. Run the narrow regression test while iterating.
2. Run the owning crate's complete test suite.
3. Run affected integration surfaces, especially LSP tests after semantic,
   module, project, cache, or diagnostic changes.
4. Run `make verify` before finalizing cross-cutting work. Use `make ci-check`
   when the request or release risk justifies the longer gate.

Run the final checks after the last edit. An earlier pass does not verify later
cleanup, docs, packaging, or conflict resolution.

## Common Commands

- Formatting: `cargo fmt --all`
- Core tests: `cargo test -p tua-core`
- LSP tests: `cargo test -p tua-lsp`
- CLI unit tests: `cargo test -p tua-cli --lib`
- CLI integration tests: `cargo test -p tua-cli --test cli`
- Lints: `cargo clippy --workspace --all-targets -- -D warnings`
- Docs build: `make docs-build`
- JSON metadata: `make json-check`
- Whitespace: `make diff-check`
- Main local gate: `make verify`
- CI-equivalent local gate: `make ci-check`

## Feature-Specific Checks

- Parser/checker/emitter: run focused `cargo test -p tua-core <filter>`, then
  `cargo test -p tua-core`.
- LSP behavior: run `cargo test -p tua-lsp` and `make lsp-features-smoke`.
- CLI/build behavior: run CLI tests and the relevant `make build-*` target.
- Examples: run `make check-example EXAMPLE=...` and `make build-example
  EXAMPLE=...`.
- Docs/nav: run `make docs-build`, `make json-check`, and `make diff-check`.
- LOVE runtime: run `make love-test` only when LOVE is available and the change
  needs runtime validation.
- VS Code packaging: run `make extension-package`, verify the resulting archive
  with `unzip -t`, and report its SHA-256. `extension-check` also validates
  `.vscode/launch.json` through `launch-config-check.js`.

## Editor Screenshot Checks

- Use `make extension-screenshots` for release-facing editor UI changes; it needs
  a local graphical editor, LÖVE, and a staged compiler. It builds the production
  bundle before capture so bundling regressions are exercised.
- Run `make extension-screenshots-check` for image links, dimensions, and hashes.
  This check is headless; it does not validate current UI behavior or appearance.
- Follow `skills/tua-release-prep/SKILL.md` for visual review and provenance.
  Record unavailable native capture as pending rather than a passing check.

## Release Notes Checks

- Before tagging a release, use `tua-doc-updater` to prepare its changelog
  summary according to `docs/releases.md`.
- Run `python3 scripts/package_release.py notes --changelog CHANGELOG.md
  --version <version> --output <temporary-notes-file>` after the final notes
  edit. Inspect the result: overview, highlights, applicable upgrade notes,
  correct tagged changelog link, and download/checksum guidance. Confirm that
  detailed changelog sections were excluded; legacy full-note fallback is not
  sufficient for a new release.
- Run `make release-tool-test docs-build diff-check` when changing the summary
  or its generation workflow. These focused checks do not replace the full
  release gate for code changes.
- Report if an existing release tag predates these changes; do not claim the
  tagged workflow will publish the updated summary until it includes them.

## Environment Failures

Distinguish a product failure from a sandbox or host limitation. For example,
the extension activation smoke test binds a loopback port and can fail with
`EPERM` in a restricted sandbox. Retry the same check with the required local
permission before reporting a regression. Never describe a partial gate as a
full pass.

## Reporting

In the final answer, mention commands run and any skipped checks. If a known
long-running or environment-dependent test was not run, say that plainly.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
