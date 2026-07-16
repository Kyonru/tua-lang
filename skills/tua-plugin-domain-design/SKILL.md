---
name: tua-plugin-domain-design
description: Design conservative Tua domain plugins. Use when proposing plugin architecture, custom LOVE-focused syntax, sprite or animation DSLs, Feather/tooling hooks, plugin diagnostics, plugin completions, lowering APIs, or any extension mechanism beyond core Lua-compatible Tua syntax.
---

# Tua Plugin Domain Design

Plugins should add domain-aware tooling, not arbitrary macro power.

## Boundaries

The implemented `TuaPlugin` surface is deliberately validation-only. Built-in
Rust plugins may contribute deterministic diagnostics over parser-owned source
and indexed asset metadata. Plugin names are stable ownership keys, and plugin
diagnostics must stay in the reserved diagnostic range.

Treat these as future design directions, not current plugin hooks:

- top-level declarations;
- domain blocks;
- structured lowering requests;
- completions, hover, signature help, and editor metadata;
- optional external-tool metadata.

Do not add one of those hooks until it has a shared cached analysis query,
deterministic ordering/conflict rules, and production consumers outside tests.

Do not allow plugins to:

- rewrite arbitrary expressions;
- change core Lua/Tua semantics;
- execute project runtime code during analysis;
- rely on raw Lua strings as the main lowering API;
- make emitted code require a hidden Tua runtime.

## Design Workflow

- Start with a built-in Rust plugin when proving the architecture.
- Prefer parser-native ordinary APIs and a validation plugin before adding new
  syntax or extension points.
- Define AST/IR inputs and structured outputs before adding syntax.
- Require deterministic diagnostics and stable spans.
- Keep third-party executable plugins disabled until the security model exists.
- Document what belongs in core versus editor/tooling adapters.

## Challenge Broad DSLs

For sprite, animation, vector, or Feather ideas, ask:

- Can this be represented as a declaration or block?
- Does the feature improve compiler and editor tooling, not just syntax style?
- Can it be lowered to readable LuaJIT/LOVE code?
- Can it be removed or changed without breaking core Tua?

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
