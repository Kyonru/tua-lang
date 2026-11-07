# Modules, imports, and declaration libraries

Bounded audit of release binary 0.6.0 using the language-auditor and Lua-defaults skills. Three medium confirmed defects; no product edits. Fixture roots are isolated single-root workspaces with explicit `source="src"`, `out="build"`. The store and ECS symptoms reproduce without the earlier fixture/root ambiguity.

## Reproduction and coverage

From repository root: `python3 internal/audits/features-2026-11-07/modules/probe.py`, then `python3 internal/audits/features-2026-11-07/modules/extra.py`, then `node internal/audits/features-2026-11-07/modules/lsp.js` and `node internal/audits/features-2026-11-07/modules/lsp.js declarations`. Python probes invoke the absolute `target/release/tua`; runtime is `/opt/homebrew/bin/lua` (5.4.8). Actual stdio LSP uses the existing LspClient. `extra.py` intentionally leaves a stale `build/normal.lua` to exercise source precedence. Domain CLI evidence precedes the extra local/inline comparison lines; original domain workflow passed check/build/runtime (`0 0 4 4`).

| Surface | Status and evidence |
|---|---|
| Static runtime import, require, renamed type re-export, mixed Lua | Passed check/build/runtime; happy prints `modules happy passed`; cli.json |
| Declaration-only `.d.tua` output exclusion | Passed: happy emits three runtime files, neither declaration emits Lua |
| Type re-export cycle | Correct TL1010, check/build fail; cycle fixture |
| Missing static module | check warns TL4002 and succeeds; build fails TL4002; recorded without asserting severity policy defect |
| Source versus stale output root precedence | LSP resolves normal to src and number, not stale string-valued build provider |
| Normal imported function navigation | definition resolves src/normal.tua function declaration |
| Unsaved provider changes | Hover changes from number signature to string signature without disk writes; dependent diagnostic republishing not established |
| Store and ECS imports | Runtime passes; editor precision failures below |
| Declaration library mapping/exclusion/runtime | check/build/runtime pass, declarations not copied; declaration and runtime source coexist |
| Runtime cycle behavior | Docs warn against cycles; not executed in this bounded pass |
| Multi-root precedence, watch, declaration unsaved propagation | Not exercised; source/build precedence only |
| SIGNALS-03 | Existing finding inspected, not independently repeated or double-counted; distinct symbol-identity issue |

## MODULES-01 — Inline require field extraction loses types

**Medium, confirmed defect.** `domains/src/main.tua:4` binds `direct = require("game").game`; LSP reports direct and direct:get() result as any, and signature help is null. The equivalent `G=require("game"); game=G.game` reports Store and the precise score/paused state. Both execute successfully. The non-domain control `inline = require("normal").inc` also reports any, while N.inc produces number. Thus this is a general expression/module-resolution gap rather than a store schema failure.

Expected: a static, literal require and field access preserve the same known export type as a named module binding. Docs/modules.md identifies static returned tables as analyzable; this is neither computed export keys nor runtime mutation. Evidence: domains-lsp.json (single root, all comparison hovers), cli.json, domains/src/*.tua. Relevant implementation: crates/tua-core/src/module_index/mod.rs:276-315 recognizes module bindings from FunctionCall expressions; crates/tua-core/src/semantic.rs:1161 supplies exports through receiver binding resolution. These are investigation entry points, not a proven complete root-cause trace.

Existing tests: module_index/tests.rs:441 exercises named require binding exports; no inline counterpart was found in this pass. Smallest maintainable improvement: resolve literal require return values in shared expression semantics, then apply ordinary field access. Regression: inline versus split binding equivalence for function and Store exports across hover, signature, completion and checker. Previously unconfirmed stores/editor-import-results.json is independently confirmed here; no existing roadmap item specifically promises this fix.

## MODULES-02 — Imported ECS component loses payload schema

**Medium, confirmed defect.** `domains/src/components.tua` exports Position with x:number. In main, `world:get(e,C.Position)` and position.x hover as any. A local identical component yields `{ x: number }` and number. Both paths run correctly. Adding an otherwise unused ecs.world() call in the provider did not restore schema, ruling out that simple initialization workaround (domains-provider-world-lsp.json).

Expected: exported static component schema survives module boundary just as exported Store state does. Evidence: domains-lsp.json, domains-original-lsp.json, domains-provider-world-lsp.json, cli.json. Relevant source: crates/tua-core/src/ecs.rs:156-187 encodes component payload in a nominal table and falls back to any when component_value_type is unavailable; module_index/mod.rs:789-810 infers exported expressions via SemanticModel, and :1230 onward resolves export aliases. The exact schema-loss stage remains unproven.

Existing module export tests cover ordinary functions and nominal classes (module_index/tests.rs:322), not imported ECS payload preservation. Smallest improvement: preserve component type metadata through the shared module export representation, avoiding an LSP-specific lookup. Regression: local, named imported, and re-exported components must produce identical world:get types and bad-field diagnostics. This confirms the existing ECS unconfirmed import symptom, so deduplicate against that feature's report. SIGNALS-03 loses navigation identity rather than this payload type; do not assert a shared root without further tracing.

## MODULES-03 — Declaration overlay disagrees across hover, signature and result inference

**Medium, confirmed defect.** `declarations/tua.toml` maps meta feel to lib.feel. Both the real runtime `src/lib/feel.lua` and typed declaration `src/vendor-types/feel.lua` are present. `F.echo` hover correctly reports `(value: string): string` and points to the declaration. Signature help reports `(value: any): any`, and result hover reports any. This violates the documented `require` declaration overlay and gives inconsistent facts within one call.

Evidence: declarations-with-runtime-lsp.json, declarations-cli.json. Removing only the runtime source still leaves the generated output provider and the bad signature (declarations-without-runtime-lsp.json). Removing both runtime source and output yields the correct string signature, although result inference remains any (declarations-no-runtime-lsp.json). Files were restored after these probes. This isolates a runtime-provider precedence discrepancy rather than malformed LuaCATS annotations. The current declarations-lsp.json is the no-runtime variant; use the explicitly named evidence files for comparisons.

Source anchors: crates/tua-core/src/analysis/project_queries.rs:1124 consults declaration_module_for_path; module_graph/mod.rs:338-352 exposes declaration lookup; semantic.rs:908 and :948 consume legacy module exports for callable typing. These identify competing resolution paths; exact internal divergence needs tracing. Existing module_graph/tests.rs:123 verifies overlay graph resolution, and CLI tests/cli.rs:2614 verifies exclusion, but neither asserts public LSP hover/signature/result agreement with a real unannotated runtime alongside declarations.

Smallest maintainable improvement: route callable/result inference through the same snapshot declaration-aware module export lookup as hover. Regression: real runtime plus declaration under mapped prefixes, assert hover, signature, result type and bad-argument diagnostics agree; repeat with stale build output. No exact pre-existing roadmap/report match found in bounded inspection.

## Limits

No cargo rebuild or full tests; existing baseline passes remain the lead's evidence. No GUI automation, LuaJIT execution, exhaustive cycles, package installation, nested multi-root, or live filesystem watch tests. Diagnostic absence after unsaved change is recorded, not elevated to a fourth defect. Source anchors locate relevant implementation but do not claim complete root-cause proof. Follow up with shared semantic/module resolution work rather than three editor-only patches.
