---
name: tua-asset-metadata
description: Improve Tua LOVE asset validation and metadata. Use when changing image/sound asset loaders, missing or unsupported asset diagnostics, asset path completion, hover metadata, build copying, source/output asset roots, or domain/editor features that depend on asset information.
---

# Tua Asset Metadata

Assets are part of the developer experience for LOVE games. Keep validation
helpful without making the compiler pretend it can run the game.

## Asset Principles

- Validate what can be proven from the project filesystem.
- Keep missing and unsupported extension diagnostics actionable.
- Use project roots, output roots, and VFS state consistently.
- Do not require real binary decoding unless the feature explicitly needs it.
- Keep build copying semantics separate from type inference.

## Metadata To Prefer

When extending asset behavior, try to produce reusable metadata:

- source span;
- normalized relative path;
- asset kind;
- extension;
- inferred LOVE type;
- validation status;
- optional dimensions or duration when safe and cheap.

The same metadata should power diagnostics, hover, completion resolve, domain
analysis, and future editor inspectors.

## Checks

- Add core tests for asset path validation and root resolution.
- Add LSP tests when metadata affects hover, completion, or diagnostics.
- Build `examples/features` when asset examples are touched.
- Avoid committing generated `build/` output unless it is an explicit fixture.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
