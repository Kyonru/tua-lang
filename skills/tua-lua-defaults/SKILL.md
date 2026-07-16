---
name: tua-lua-defaults
description: Preserve automatic Lua developer features in Tua. Use when working on Lua interop, LuaHelper/LuaLS-style annotations, standard library stubs, require resolution, .lua files in projects, Lua completions, Lua globals, module exports, or preventing .tua language mode from losing baseline Lua tooling behavior.
---

# Tua Lua Defaults

Tua is typed Lua on top of Lua, so `.tua` files should feel like Lua files with
extra type intelligence, not like a language that forgot Lua exists.

## Defaults To Preserve

- Lua keywords, globals, and standard libraries should complete and hover.
- `.lua` files should be copied unchanged during builds and indexed for simple
  module exports.
- `require("...")` should preserve runtime semantics while powering editor
  navigation and completions.
- LuaDoc/LuaHelper/LuaLS-style annotations should enrich `.lua` and `.tua`
  files where supported.
- Unknown dynamic Lua should stay permissive instead of producing confident but
  wrong diagnostics.

## Compare Before Inventing

Before adding a Lua-facing feature, check the useful behavior from:

- LuaHelper/LuaLS annotation patterns;
- rust-analyzer navigation/completion patterns;
- existing Tua docs: `docs/lua-interop.md`, `docs/capabilities.md`, and
  `docs/lsp-parity.md`;
- current priorities in `docs/roadmap.md` and boundaries in
  `docs/architecture.md`.

Do not clone every LuaHelper diagnostic or setting. Keep what improves normal
LOVE/LuaJIT development.

## Common Pitfalls

- Treating `.tua` as a separate language mode with no Lua completions.
- Fixing `.tua` interop while leaving `.lua` module providers untyped.
- Adding project-wide behavior that scans arbitrary directories on each
  request.
- Reporting errors for dynamic Lua that cannot be proven statically.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
