---
name: tua-domain-design
description: Design first-class Tua game domains. Use when proposing or changing ECS, sprite, animation, vector, Feather/tooling metadata, domain diagnostics, domain completion, or compiler-owned lowering beyond baseline Lua-compatible Tua syntax.
---

# Tua Domain Design

Game domains should deepen compiler and editor understanding without creating a
generic executable extension system.

## Boundaries

- Keep domain behavior in `tua-core` modules with typed inputs and outputs.
- Run diagnostics through the cached `AnalysisFile` build.
- Reuse parser metadata, semantic queries, asset indexes, and the structured
  lowering operation IR.
- Prefer ordinary Lua-compatible APIs such as `spriteSheet({...})` and
  `ecs.world()` over new syntax.
- Keep opt-in domains disabled through project configuration, not dynamic
  registration.
- Give each expensive domain stage its own profiler span.

Do not let a domain:

- rewrite arbitrary expressions;
- change core Lua/Tua semantics;
- execute project runtime code during analysis;
- emit raw Lua strings outside compiler-owned lowering;
- make emitted code require a hidden Tua runtime.

## Design Workflow

1. Define the user-facing ordinary Lua API.
2. Identify the parser, semantic, asset, and project facts it needs.
3. Add typed domain queries or metadata only where existing shared facts are
   insufficient.
4. Route diagnostics through analysis and generated code through validated
   lowering operations.
5. Add completion, hover, signature help, and navigation from the same semantic
   facts.
6. Measure the domain stage and test disabled/enabled behavior.

## Challenge Broad DSLs

For sprite, animation, vector, ECS, or Feather ideas, ask:

- Can an ordinary Lua-compatible call or table represent this clearly?
- Does the feature improve compiler and editor tooling, not just syntax style?
- Can it lower to readable LuaJIT/LOVE code?
- Can it remain isolated and optional without an extension registry?

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
