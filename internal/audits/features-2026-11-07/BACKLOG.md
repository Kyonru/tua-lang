# Prioritized improvement backlog

Effort estimates are engineering ranges for implementation plus focused regression coverage, not commitments. Finding IDs link through [FINDINGS.md](FINDINGS.md) to exact reproductions and source references. Preserve separate regression cases even where a shared fix resolves several symptoms.

## Correctness work, in recommended order

| Order / effort | Findings | Shared fix direction | Acceptance criteria / dependencies |
|---|---|---|---|
| 1 — 2–4 days | ASSETS-01, PROJECTS-01; then ASSETS-02 | Track generated-file ownership with canonical output-root identity and verified cache content. Cleanup and materialization must respect ownership. | Disposable user files survive root changes, clean/rebuild and declaration cleanup; cache tampering causes rebuild. Test path/root changes and legitimate generated cleanup together. |
| 2 — 1–3 days | FMT-01, LINTER-01 | Preserve lexical literal spans and statement-level edit boundaries; avoid trimming/deleting whole lines. | Long-string runtime bytes preserved across LF/CRLF; local fix and fix-all retain adjacent statements and Unicode ranges. CLI/LSP formatting agree. Independent of order 1. |
| 3 — 2–5 days | CLASS-01, VECTORS-01/02, CAMERA-01 | Lower references using resolved symbol/receiver identity; determine helper requirements from syntax/semantic use rather than spelling searches. | Self identity holds with flattened constructors; aliases, bracket members, comments and chained calls emit required helpers without duplicate injection. Keep distinct lowering/injection implementations visible in review. |
| 4 — 3–6 days | TYPING-01/02/03/04, FUNCTIONS-01/02/03, SIGNALS-02, POOLS-01, TWEEN-02, LOVE-01 | Centralize assignment and callable contracts: dictionary writes, nil padding, callback variance, optional writes, defaults, return-expression checking and overload selection. | Each previously accepted bad program rejects meaningfully; valid shortened callbacks/default nil/optional deletion pass. Dot/bracket and checker/editor contracts agree. Retain positive controls for Lua behavior. |
| 5 — 2–4 days | MODULES-01/02/03, LUA-INTEROP-01/02, SIGNALS-03 | Carry declaration/schema/overload identity through imported values and metatable receivers. | Inline and two-step require agree; exported ECS schemas and signal navigation survive import; hover/signature/result/completion agree; imported colon-call arity matches Lua runtime. Build on shared contract tests from order 4. |
| 6 — 2–4 days | SPRITES-01/02/03, INPUT-01/02, ECS-01/02/03 | Validate domain schemas and numeric constants consistently; model absence and false separately; bound animation progress. | Zero/nonpositive durations cannot hang update; multi-wrap counts and pause behavior match contract; numeric spelling cannot bypass validation; false round-trips and absent components are nil-aware. SPRITES-01 should be pulled forward as an independent release blocker. |
| 7 — 3–6 days | STORES-03, SIGNALS-01, SM-01/02/03/04, POOLS-02, TWEEN-01 | Specify state transitions and callback reentrancy, then mutate ownership/state atomically around callbacks. | Once fires once under recursion; cancelled transitions have documented effects; nested async transitions survive; pool counts stay valid and concurrent leases unique; callback cancellation is safe. Preserve synchronous error behavior intentionally. |
| 8 — 2–4 days | AUDIO-01/02/03/04/05 | Enforce overlapping caps together; use interval crossings and loop-aware timing; return detached snapshots. | Global and per-cue bounds both hold, long frames/loop wraps emit correct beat/bar events, loop regions work without BPM, snapshot mutation cannot alter internal state. Define parent-bus policy separately. |
| 9 — 2–3 days | CAMERA-02/03/04/05, VECTORS-03/04 | Share effective camera transforms and numerically stable vector operations. | Rotated/shaken bounds cover rendered view; conversions agree at odd dimensions; preset matches docs; tiny/large finite vectors normalize without false zero. Add eventual native pixel checks. |
| 10 — 2–4 days | WATCH-01/02, API-MCP-01/02/03, DBG-01 | Treat project identity/configuration/overlays as coherent snapshots; use robust source-path parsing and protocol validation. | Output-root reload moves all runtime artifacts; newly created conf.lua is watched; revision IDs distinguish changed snapshots; unsaved files appear in diagnostics; paths with spaces remap; JSON-RPC notifications are not answered. |
| 11 — 1–3 days | EDITOR-01/02, SHADERS-01, TESTING-01 | Use structured signatures/documentation and backend provenance, not rendered-string parsing. | Colon argument candidates exclude receiver fields; Markdown fences render once; macro uniform names match valid shader behavior; startup stderr survives structured test reporting. |
| 12 — 0.5–1 day | PACKAGING-01, SETUP-DOCS-01 | Align distributed metadata and setup documentation with shipped behavior. | Fused app has no unintended LOVE document association; setup guide describes all five steps and optional downloads. |

## Missing capabilities and usability

These are not additional confirmed defects. Implement only after the corresponding contracts are stable.

| Opportunity | Dependency / effort | Acceptance criterion |
|---|---|---|
| Contextual ECS system and LOVE callback types (ECS-GAP-01, LOVE-02) | Shared imported schema/callable resolution; 2–4 days | Unannotated callback parameters show useful types consistently in diagnostics, hover and completion. |
| Literal-sensitive overload ranking (EDITOR-G01, also input observation) | Structured overload resolver; 1–3 days | Same-arity literal arguments select appropriate signature and completions while incomplete inputs remain usable. |
| Reactive explorer cache and Lua file visibility (EXPLORERS-01/02) | Project snapshot invalidation; 1–3 days | Unsaved edits, create/delete/rename and mixed Lua sources update graphs and sidebar predictably; test provider and real editor. |
| Store selectors/batching, pool prewarming/capacity policy, richer migration tools | API design required; estimate after scope | Document observable subscription/ownership semantics first; avoid silently changing current runtime behavior. |
| Protocols/interfaces, class generics/abstract methods | Language design decision; not estimated | Written type/runtime compatibility proposal and cross-module/editor acceptance suite before implementation. |
| Explicit import cleanup side effects (LINTER-02), audio parent-bus caps | Policy decision; small implementation only after agreement | Document whether initialization effects/order and inherited limits are guaranteed; keep safe fix-all conservative. |

## Performance and qualification

The audit does not establish a new comprehensive latency baseline. Before optimizing, measure cold/warm completion, incomplete-edit completion, imported-module edits and format-on-save against representative projects with fixed machines and percentiles. Preserve prior roadmap targets; do not infer regression or improvement from these functional probes.

Add installed-editor integration for Test Explorer, sidebar refresh, completions and inline acceptance; native device/audio and camera pixel checks; long-session watch/cache tests; Windows/Linux packaging/install checks; and real Ollama cancellation/output tests when a model is available. These are verification tasks, not confirmed failures. Use the fixture corpus here as regression input, migrating small stable cases into normal tests rather than importing large generated audit logs.

## Release gate

Re-run focused regressions after each coherent fix, then the normal workspace, runtime, editor and packaging gates. Resolve or explicitly disposition the existing nominal-type failures and inline snapshot mismatch separately. No new feature or passing targeted audit should be used to waive destructive-output or behavior-changing-edit failures.
