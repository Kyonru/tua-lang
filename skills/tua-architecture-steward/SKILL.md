---
name: tua-architecture-steward
description: Guide architectural changes in the Tua Rust workspace. Use when modifying crate boundaries, analysis snapshots, VFS/project-model/HIR/query-cache work, LSP request routing, shared semantic infrastructure, or any change that risks creating parallel one-off code paths instead of maintainable architecture.
---

# Tua Architecture Steward

Use architecture changes to reduce duplication and make future features easier,
not to reshuffle code for its own sake.

## First Checks

- Read the relevant roadmap phase, especially Phase 12 in `ROADMAP.md`.
- Inspect the current implementation before proposing new modules.
- Identify whether the request is a local bug, a missing shared abstraction, or
  a project-model gap.
- Prefer evolving existing crates:
  - `tua-core` owns language, analysis, diagnostics, and IDE queries.
  - `tua-lsp` owns protocol conversion and server state.
  - `tua-cli` owns command orchestration and filesystem output.

## Architecture Rules

- Keep LSP handlers thin: protocol conversion plus snapshot access.
- Avoid request-time disk reads once project analysis can answer the query.
- Prefer one shared semantic path over per-feature fixes.
- Preserve public helper APIs such as `*_tua_source` when useful for tests.
- Keep emission and provenance together in `LuaEmission`; CLI source maps must
  consume compiler provenance rather than reconstructing it from generated Lua.
- Route source-backed emission changes through validated `LoweringOperation`s;
  do not add another whole-document text adapter or expose plugin lowering that
  can overlap core operations.
- Preserve parser normalization provenance when generated syntax changes line
  positions; emission must consume syntax-backed provenance rather than scan
  generated text for helper or declaration markers.
- Use stable IDs and source spans when a feature needs navigation, rename,
  references, or edits.
- Keep dynamic Lua gradual: represent unknown or `any` explicitly instead of
  guessing.

## Challenge Broad Ideas

Push back when a proposal:

- adds a new runtime or changes LuaJIT/LOVE compatibility;
- invents syntax before a project model can support it;
- patches hover/completion/checker separately for the same semantic gap;
- duplicates LuaHelper or rust-analyzer wholesale instead of extracting the
  useful pattern.

Offer a smaller architectural step that can be tested and committed.

## Expected Finish

- Update tests around the architecture boundary touched.
- Update docs or roadmap if the architecture surface changes.
- Run the narrowest meaningful checks, then broader checks when the blast radius
  crosses crate boundaries.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
