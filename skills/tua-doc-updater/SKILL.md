---
name: tua-doc-updater
description: Update Tua documentation and examples whenever implementation, CLI behavior, LSP capabilities, diagnostics, LOVE helpers, configuration, product priorities, or docs navigation change. Use to proactively find every affected doc surface, keep docs factual, add or update commented examples as the project grows, and verify the Zensical docs site.
---

# Tua Doc Updater

Every user-visible change should leave matching docs behind. Do not wait for a
separate docs request when the implementation changed.

## Check These Surfaces

- `docs/specification.md` for the canonical language/toolchain contract.
- `ROADMAP.md` for pending repository priorities.
- Focused pages under `docs/` for current language, editor, CLI, configuration,
  and workflow behavior.
- `docs/archive/` only for dated historical snapshots; do not update archived
  ledgers as if they were current plans.
- `docs/` for user-facing documentation.
- `zensical.toml` for docs navigation.
- `examples/` READMEs and sources for runnable, commented examples.
- `README.md` for the project-level summary.

## Update Rules

- If behavior changed, update the relevant docs in the same work.
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
- If the spec contract changed, update `docs/specification.md` before or with
  user-facing docs.
- If implementation changes current support, update `docs/specification.md`
  and the focused feature page. Update the diagnostic-code reference when
  codes, severity behavior, settings, or fixes change.
- If product priorities change, update the relevant outcome in root
  `ROADMAP.md`. Do not add planning matrices to the public docs site.
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
  planning references when renaming or reframing features.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
