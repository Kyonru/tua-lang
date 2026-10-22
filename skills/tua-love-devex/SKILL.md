---
name: tua-love-devex
description: Improve the LOVE-focused Tua developer experience. Use when changing LOVE API completion, hover, signature help, method colon guidance, constructors, object method return types, asset helpers, LOVE examples, or features intended to make Tua better for LOVE2D game projects.
---

# Tua LOVE DevEx

Optimize for what a LOVE developer expects while keeping emitted Lua ordinary.

## Expected LOVE Behavior

- `love.graphics.newImage(...)` and `image "..."` should infer `Image`.
- `love.audio.newSource(...)` and `sound "..."` should infer `Source`.
- Known LOVE object methods should complete after `:` and, where helpful,
  rewrite accidental `.` completion sites to `:`.
- Dot calls to known LOVE methods should warn because `self` is not passed.
- Method signatures should expose parameter names, enum-like values, docs, and
  return types.
- Constructor propagation should work through locals, simple functions, and
  required module exports.

## Design Heuristics

- Prefer generated LOVE API catalog data over hand-written one-offs.
- Add common LOVE object types incrementally, but through the same resolver path.
- Keep asset helper syntax small and obvious.
- Update `examples/features` and LOVE examples when behavior is user-visible.
- Treat missing autocomplete as a semantic-model bug if the type should already
  be known.

## Game-Scale DevEx Trials

For a temporary LOVE game used to evaluate Tua:

- exercise editing as well as compilation: completion, hover, signature help,
  definitions, references, diagnostics, formatting, and source maps;
- run `tua check`, build the project, and use a real LOVE runtime smoke test
  when LOVE is installed;
- measure representative update/draw and allocation-sensitive paths rather than
  judging only toy snippets;
- validate editor request positions before treating an empty response as a Tua
  issue;
- label simulated developer personas honestly and keep temporary experiment
  artifacts out of production commits when requested.

Convert confirmed findings into focused core/LSP/runtime regression tests. Do
not make the experimental game itself the only coverage.

## Challenge Scope

If a request asks for a large LOVE feature, split it into:

- type/catalog support;
- parser/lowering support if new syntax is needed;
- diagnostics;
- LSP/editor metadata;
- examples and docs.

Do not add a LOVE-specific special case to one LSP feature if the same fact
should power hover, completion, signature help, and diagnostics.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
