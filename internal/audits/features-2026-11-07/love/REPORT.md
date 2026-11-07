# LOVE API/catalog/callback feature audit

Bounded pass against baseline release binary 0.6.0; no product edits. One medium confirmed checker defect and one low authoring feature gap. This is not an exhaustive catalog or platform certification.

## Reproduction and evidence

From repository root:

```
node internal/audits/features-2026-11-07/love/probe.js
(cd internal/audits/features-2026-11-07/love/runtime && /Users/kyonru/Documents/projects/Experiments/tlua/target/release/tua build)
/Applications/LOVE.app/Contents/MacOS/love internal/audits/features-2026-11-07/love/runtime/build
```

`evidence.json` preserves complete CLI results and actual stdio LSP responses/positions. `probe.log` is a summary. Fixtures are under `objects/`, `callbacks/`, `colors/`, `callbackcompletion/`, and `runtime/`; generated runtime Lua is retained. LOVE required sandbox escalation to initialize: initial sandbox launch exited 134 with empty `runtime.log`; authorized unsandboxed launch exited 0. `runtime-unsandboxed.log` reports LOVE 11.5.0 and all assertions passing. `runtime-build.log`: built two files. No cargo rebuilds, full suites, or desktop editor automation.

| Surface | Status / result |
|---|---|
| Canvas, Font, Text, Source constructors | CLI/LSP exercised; Canvas/Source types inferred |
| Canvas inherited Texture method | Completion, hover, signature parameter 1, documentation exercised |
| Multi-return getDimensions | Both width and height hover number |
| Drawable arguments and catalog overloads | Canvas/Text draw accepted; signature overload lists returned; color table overload present (default active signature remains numeric at cursor before argument) |
| Dot/colon | TL4018 matches CLI/LSP; resolving completion adds exact dot-to-colon edit |
| Builtin navigation | Chained constructor getWidth definition returns `tua:/builtins/Canvas.getWidth.tua`; `tua/builtinDocument` returns method source, docs and wiki URL |
| Callback declarations | Omitted parameters accepted; typo, incompatible type, and extra required parameter diagnosed TL5006 |
| Callback snippet | Resolved update item inserts `update(dt)` body and carries parameter documentation |
| LoveColor | Numeric RGB accepted; string component rejected TL1003; length/bounds remain permissive as explicitly documented |
| Void chain | Empty completion and null signature, but checker accepts runtime-invalid call (LOVE-01) |
| Actual runtime | Canvas creation/filter/dimensions, Text creation/mutation, color table and draw all pass in LOVE 11.5 |
| Source playback / devices | Editor/catalog only; no audio hardware or playback test |
| Every overload/enum/platform API | Not exhaustively tested |
| VS Code reference webview / links on external wiki | Source docs inspected; not launched or browsed |

## LOVE-01 — Known void LOVE calls lose their invalid-chain diagnostic

**Medium; confirmed defect; new checker finding with an already-known editor negative control.**

Exact reproduction: `objects/src/main.tua:10` contains `text:set("x"):getWidth()`. `tua check` exits 0 (only unrelated dot-call warning on line 9). Actual LSP diagnostics likewise omit this call. The standalone runtime project builds successfully and catches the same operation at `runtime/src/main.tua:14`; actual LOVE reports `attempt to index a nil value` at emitted `main.lua:14`.

Expected: a known no-value result should diagnose member access/call; unknown dynamic returns can remain permissive. Actual: CLI silently accepts this statically known failure, while completion returns `[]` and signature help `null` after `Text:set`.

Source: `crates/tua-core/src/check/mod.rs:1825` converts an empty first-return vector into `None`; `:1840–1888` treats a missing receiver/callable as dynamic, inferring arguments without checking the operation. The checker comment at `:1809` describes unknown-chain fallback, but a catalog-known void return is not unknown.

Blind spot: `crates/tua-core/src/types/tests.rs:1599` explicitly covers this exact chain only by expecting absent/any inference. `crates/tua-core/src/analysis/tests.rs:7786` checks empty completion at the void boundary, not checker rejection. Prior hover audit calls it a correct negative editor control (`internal/audits/devex-2026-11-06/hover/REPORT.md:118,366`); it does not establish checker/runtime correctness. Separate from prior helper-chain signature issues and conservative empty unannotated function stubs.

Smallest improvement: preserve known-void versus unknown call outcomes in shared chain resolution and report known invalid member/call operations in checking. Keep editor suppression after void and permissive truly dynamic calls. Regression: assert CLI/core diagnostic for `Text:set(...):getWidth()` plus positive constructor/getDimensions chains and unknown-return controls; retain a LOVE runtime oracle where available.

Evidence: `evidence.json` objects responses/CLI; `runtime/src/main.tua`; `runtime/build/main.lua`; `runtime-build.log`; `runtime-unsandboxed.log`.

## LOVE-02 — Catalog callback types are not contextual body parameter types

**Low; feature gap; not a claimed violation of documented inference.**

`callbacks/src/main.tua:2–6` declares ordinary unannotated `love.update(dt)` and `love.keypressed(key, scancode, isrepeat)`. Actual hover returns `parameter dt: any`, `parameter key: any`, `parameter isrepeat: any`. Resolved callback completion simultaneously advertises `dt: number`. Explicitly annotated incompatible callbacks are correctly rejected.

Expected improvement: reuse catalog callback parameter types inside recognized callback bodies, allowing key/string and delta-time authoring assistance without repeating annotations. Current docs (`docs/language.md:773–780`, `docs/lsp.md:477–486`) promise declaration checking and snippets, not this inference; therefore this is a feature gap, not a confirmed defect.

Source: `crates/tua-core/src/check/mod.rs:935–989` loads the generated callback signature for validation separately from the declared function environment. Tests `crates/tua-core/src/check/tests.rs:1802–1863` cover declaration names, omitted parameters, overload signatures and explicit incompatibility; they do not require unannotated callback-body hover types.

Smallest improvement: provide recognized catalog callback context during shared function parameter binding, preserving explicit annotations, supported callback overloads and shadowing. Add semantic hover/type tests for update/keypressed and retain the existing declaration validation tests. No matching item found in ROADMAP or prior audit reports searched; ROADMAP:28 already prioritizes one generated LOVE source, which should own this metadata.

Evidence: `evidence.json` callback-dt/callback-key/callback-repeat and callback-snippets-resolved; `callbacks/src/main.tua`.

## Options and limits

No additional configuration is needed for tested built-in LOVE catalog features. The nonstandard native executable path works directly; editor configuration documents `tua.love.command`. LoveColor length/bounds are an explicit non-goal of current checking, not a missing toggle defect. The native LOVE runtime test mode remains a known roadmap item; this audit uses an isolated direct launch. Shader tools, asset metadata/copying, hot reload, packaging, helper runtimes, and broad Lua interop belong to other lanes. External wiki URL correctness and offline webview interaction were not verified. API coverage was representative, not every generated entry.
