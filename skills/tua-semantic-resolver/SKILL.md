---
name: tua-semantic-resolver
description: Centralize Tua semantic behavior. Use when fixing or designing type inference, function returns, multi-return values, module export types, LOVE object methods, field-chain resolution, hover, completion, signature help, inlay hints, definitions, diagnostics, or any repeated bug where one surface was fixed but another still returns any or stale results.
---

# Tua Semantic Resolver

When a semantic bug appears in more than one editor/checker surface, solve the
resolver problem instead of patching the visible symptom.

## Diagnosis

- List every affected surface: checker, hover, completion, signature help,
  inlay hints, definition, references, rename, semantic tokens, and LSP.
- Find the shared source of truth currently used by those surfaces.
- If behavior differs by surface, route them toward `SemanticModel`,
  `TypeEnvironment`, module index, or the Phase 12 analysis snapshot.
- Check whether required modules, LOVE constructors, asset helpers, multi-return
  values, or `any` fallback are involved.

## Implementation Rules

- Infer locals, function returns, receiver types, field chains, call signatures,
  and module exports in one reusable path.
- Preserve multi-return information where Lua/LOVE APIs return multiple values.
- Keep `any` permissive and quiet.
- Treat required `.lua` modules and `.tua` modules through the same module export
  path when possible.
- Prefer adding resolver tests before LSP protocol tests.

## Regression Shape

For each semantic fix, add tests for at least two surfaces when practical:

- core type/check behavior;
- hover or inlay hint;
- completion or signature help;
- LSP behavior if protocol conversion was touched.

If a bug was caused by duplicated inference paths, add a test that would fail if
only one surface were fixed.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
