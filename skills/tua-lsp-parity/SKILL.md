---
name: tua-lsp-parity
description: Shape Tua language-server work using LuaHelper and rust-analyzer patterns. Use when adding or fixing diagnostics, hover, completion, signature help, definitions, references, rename, document symbols, workspace symbols, semantic tokens, inlay hints, code actions, call hierarchy, or automatic Lua features in the LSP.
---

# Tua LSP Parity

Use mature editors as references without copying their entire scope.

## Reference Mindset

- LuaHelper/LuaLS show what Lua developers expect by default.
- rust-analyzer shows useful architecture and UX patterns.
- Tua should keep Lua defaults, add type-aware help, and stay conservative with
  dynamic Lua.

## Before Implementing

- Check `docs/lsp.md`, `docs/language.md`, and root `ROADMAP.md` when the
  work affects a current priority.
- Identify the semantic source of truth before changing protocol output.
- Prefer core query tests before LSP JSON tests.
- Keep `.lua` and `.tua` behavior aligned where the feature is language-neutral.
- Return empty/null for unknown or unsafe dynamic cases instead of guessing.

## LSP Quality Bar

- Diagnostics, hover, completion, signature help, inlay hints, and definitions
  should agree on types.
- Completion resolve should avoid stale, oversized eager payloads.
- Watched file changes should update required-module behavior.
- VS Code behavior should be smoke-tested when activation or protocol shape
  changes.
- Fast current-file diagnostics should defer facts that require the project
  model instead of publishing errors that disappear after project analysis.

## Probe Correctness

- Build request positions from the exact document text and LSP encoding. For a
  multiline search match, compute the line and character from the final byte
  offset; do not add a flat offset to the first line.
- Verify the request payload and cursor location before classifying a failed
  editor replay as a product defect.
- Prefer direct LSP requests for completion, hover, signature help, definition,
  and diagnostics, then add a VS Code integration test only for client-specific
  behavior.
- Keep harness defects separate from product findings in reports and issue
  lists.

## Verification

- Run focused `cargo test -p tua-core <feature>` tests.
- Run `cargo test -p tua-lsp`.
- Run `make lsp-features-smoke` when editor-visible behavior changes.
- Run `make verify` when shared semantic, project, cache, or diagnostic staging
  behavior changes across crates.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
