---
name: tua-doc-updater
description: Update Tua documentation whenever implementation, examples, CLI behavior, LSP capabilities, diagnostics, LOVE helpers, configuration, roadmap phases, or docs navigation change. Use to proactively find every affected doc surface, keep docs factual, update examples when needed, and verify the Zensical docs site.
---

# Tua Doc Updater

Every user-visible change should leave matching docs behind. Do not wait for a
separate docs request when the implementation changed.

## Check These Surfaces

- `SPEC.md` for language/toolchain contract.
- `ROADMAP.md` for commit-sized plans.
- `docs/` for user-facing documentation.
- `zensical.toml` for docs navigation.
- `examples/` READMEs and sources for runnable examples.
- `README.md` and `TODO.md` for project-level summaries.

## Update Rules

- If behavior changed, update the relevant docs in the same work.
- If examples demonstrate the behavior, update example sources and READMEs.
- If CLI output, config keys, diagnostics, or LSP capabilities changed, update
  the matching reference page.
- If the spec contract changed, update `SPEC.md` before or with user-facing
  docs.
- If a roadmap phase changed, update `docs/roadmap.md` and parity docs when
  they reference shifted item numbers.
- If a new docs page is added, add it to `zensical.toml`.
- If an example source changes, rebuild or explain why generated output is not
  committed.
- Keep the docs factual: label future work as future work.
- Remove stale language such as "not implemented" after behavior lands.

## Verification

- Run `make docs-build` after docs or nav changes.
- Run `make diff-check`.
- Run example build/check targets when example docs or source changed.
- Search for stale terms such as `tlua`, `scaffold`, `not implemented`, and old
  roadmap item numbers when renaming or reframing features.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
