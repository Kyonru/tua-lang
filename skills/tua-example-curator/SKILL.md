---
name: tua-example-curator
description: Keep Tua examples runnable, representative, and aligned with docs. Use when editing examples/features, examples/love-minimal, examples/love-full, generated example output, LOVE assets, interop samples, or docs that describe bundled examples.
---

# Tua Example Curator

Examples are test fixtures and user documentation at the same time.

## Example Rules

- Keep `examples/features` clean under `tua check`.
- Keep LOVE examples runnable or explicitly smoke-testable.
- Use examples to demonstrate real supported behavior, not desired future
  behavior.
- Keep placeholder assets when static asset validation needs files.
- Avoid committing generated `build/` output unless the repo already treats it
  as an explicit fixture.

## What To Check

- Source examples match docs and README descriptions.
- `.lua` interop files are copied or required as expected.
- LOVE method calls use `:` when the receiver is a known LOVE object.
- Asset helper examples have existing paths and supported extensions.
- Feature examples cover new user-facing capabilities when reasonable.

## Verification

- `make check-example EXAMPLE=examples/features`
- `make build-features`
- `make build-love-minimal` or `make build-love-full` when touched.
- `make lsp-features-smoke` for editor behavior demonstrated by examples.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
