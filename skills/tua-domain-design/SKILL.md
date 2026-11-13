---
name: tua-domain-design
description: Design or change first-class Tua game APIs and compiler support, including sprites, ECS, behavior trees, audio, camera, tweening, and procedural helpers. Use for new domain behavior or lowering, not ordinary game code that composes existing features.
---

# Tua Domain Design

Game domains should deepen compiler and editor understanding without creating a
generic executable extension system.

## Boundaries

- Keep compiler-owned domain facts in `tua-core`; bundled Lua runtime modules
  own execution. Do not implement feature semantics in the extension.
- Run diagnostics through the cached `AnalysisFile` build.
- Reuse parser metadata, semantic queries, asset indexes, and the structured
  lowering operation IR.
- Prefer ordinary Lua-compatible APIs such as `spriteSheet({...})` and
  `ecs.world()` over new syntax.
- Keep helper runtimes opt-in by source references. In `.tua`, supported
  built-ins inject their import only when used; a compatible project module
  can replace the bundled runtime. Keep configuration gates for features that
  actually require configuration, such as external asset pipelines or previews.
- Give each expensive domain stage its own profiler span.

Do not let a domain:

- rewrite arbitrary expressions;
- change core Lua/Tua semantics;
- execute project runtime code during analysis;
- emit raw Lua strings outside compiler-owned lowering;
- add a required global runtime to ordinary typed Lua that uses no helper.

## Design Workflow

1. Define the user-facing ordinary Lua API.
2. Identify the parser, semantic, asset, and project facts it needs.
3. Add typed domain queries or metadata only where existing shared facts are
   insufficient.
4. Route diagnostics through analysis and generated code through validated
   lowering operations.
5. Add completion, hover, signature help, and navigation from the same semantic
   facts.
6. Test helper-present and helper-absent output, runtime replacement, and
   composability with neighboring game features. Measure stages that can
   affect typing or build latency.

## Challenge Broad DSLs

For sprite, animation, ECS, audio, procedural, or other game APIs, ask:

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
