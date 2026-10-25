---
name: tua-doc-updater
description: Update Tua documentation and examples whenever implementation, CLI behavior, LSP capabilities, diagnostics, LOVE helpers, configuration, product priorities, or docs navigation change. Use to proactively find every affected doc surface, keep docs factual, add or update commented examples as the project grows, and verify the Zensical docs site.
---

# Tua Doc Updater

Every user-visible change should leave matching docs behind. Do not wait for a
separate docs request when the implementation changed.

## Check These Surfaces

- The focused pages under `docs/` (`language.md`, `typing.md`, `cli.md`,
  `lsp.md`, `linter.md`, and the feature pages), which together hold the
  language and toolchain contract; each topic has one owning page.
- `ROADMAP.md` for pending repository priorities.
- Focused pages under `docs/` for current language, editor, CLI, configuration,
  and workflow behavior.
- `docs/archive/` only for dated historical snapshots; do not update archived
  ledgers as if they were current plans.
- `internal/` for maintained contributor workflows, implementation plans,
  benchmark methodology, and evaluation evidence that should not appear in the
  public site navigation.
- `docs/` for user-facing documentation.
- `zensical.toml` for docs navigation.
- `examples/` READMEs and sources for runnable, commented examples.
- `README.md` for the project-level summary.

## Update Rules

- If behavior changed, update the relevant docs in the same work.
- Put release notes in the top `Unreleased` section of `CHANGELOG.md`; verify
  the surrounding heading instead of inserting near an older matching entry.
- For every user-facing language, CLI, LSP, LOVE, Lua interop, or build change,
  decide whether `examples/` needs a new sample or an update to an existing
  sample. Prefer updating `examples/features` for focused feature coverage.
- Add comments in example source when possible, using Lua/Tua `--` comments to
  name the feature being demonstrated near the relevant code.
- If a feature is better shown in a LOVE project, update `examples/love-minimal`
  or `examples/love-full` instead of adding an artificial features sample.
- If examples demonstrate the behavior, update both example sources and their
  README summaries.
- If CLI output, config keys, diagnostics, or LSP capabilities changed, update
  the matching reference page.
- If the language or toolchain contract changed, update the page that owns
  that topic in the same work.
- If implementation changes current support, update the focused feature page.
  Update the diagnostic-code reference when
  codes, severity behavior, settings, or fixes change.
- If product priorities change, update the relevant outcome in root
  `ROADMAP.md`. Do not add planning matrices to the public docs site.
- If a new docs page is added, add it to `zensical.toml`.
- If an example source changes, rebuild or explain why generated output is not
  committed.
- Keep the docs factual: label future work as future work.
- Keep public pages task-focused. Move phase histories, tuning thresholds,
  repository test mechanics, and maintainer-only instructions to `internal/`
  and link them from `CONTRIBUTING.md` when they remain useful.
- Prefer one owning explanation and concise cross-links over repeating the same
  feature overview across the homepage, language guide, LSP guide, and focused
  feature page.
- Label simulated persona evaluations and synthetic editor replays explicitly;
  do not present them as human studies. Separate product findings from harness
  defects and distinguish fixed issues from remaining gaps.
- Remove stale language such as "not implemented" after behavior lands.

## Editor Screenshots

For editor UI changes and release preparation, follow
`skills/tua-release-prep/SKILL.md`. Use `make extension-screenshots` to rebuild and
capture the actual extension, then inspect every PNG and update README captions.
Run `make extension-screenshots-check` to verify the completed image set and links.
Do not substitute static mockups for screenshots of supported features.

## Release Summaries

- When preparing a release, write or refresh `### Release summary` at the
  start of the relevant changelog entry before creating the release tag.
  Keep detailed changes below the next level-three heading.
- Follow `docs/releases.md`: aim for 150–300 words, with a short overview,
  `#### Highlights` containing three to six user-facing bullets, and
  `#### Upgrade notes` when needed. Include material compatibility and security
  changes; summarize only changes supported by the detailed entry.
- Maintain the summary under `Unreleased` until release preparation moves the
  entry to its version. Do not require a summary for every individual fix.
- Generate GitHub notes with `scripts/package_release.py notes`; do not keep
  a second hand-maintained summary. The generator adds the tagged changelog
  link and download/checksum guidance.
- If the release is already tagged, report whether the tag contains the new
  summary. Do not silently move a tag or edit a published release.

## Verification

- Run `make docs-build` after docs or nav changes.
- Run `make diff-check`.
- Run example build/check targets when example docs or source changed.
- Search for stale terms such as `tlua`, `scaffold`, `not implemented`, and old
  planning references when renaming or reframing features.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
