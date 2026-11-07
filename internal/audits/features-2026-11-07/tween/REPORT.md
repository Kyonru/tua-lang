# Tweening audit

The representative typed Tua workflow builds and runs correctly: typed numeric target, linear interpolation, start/update/complete callbacks, shorthand chaining, independent group, and zero-duration completion. Runtime replacement is honored. Two medium confirmed defects remain: unsafe callback cancellation in the bundled upstream runtime, and an annotation that fails to distinguish `after` overloads.

## Environment and reproduction

Baseline: `../BASELINE.md`, release binary 0.6.0; no product changes. Commands from repository root:

```sh
/opt/homebrew/bin/lua internal/audits/features-2026-11-07/tween/runtime.lua
node internal/audits/features-2026-11-07/tween/probe.js
```

For each of `positive`, `invalid`, `after_invalid`, `replacement`, run the absolute `target/release/tua build` binary from that fixture directory. For successful builds, run `/opt/homebrew/bin/lua main.lua` from its `build/` directory. Preserved build/runtime logs contain exact diagnostics and exit statuses. Lua 5.4.8 was exercised; this focused pass did not repeat LuaJIT/LOVE execution. Existing baseline includes standalone runtime suite coverage.

| Surface | Coverage | Result |
| --- | --- | --- |
| Typed target, interpolation, callbacks, chaining | Built and ran `positive` | Pass; outputs 5/10 then 10/0, independent group sets x=20 |
| Easing names and exact completion, delays, groups, stop/remove | Existing `vendor/tests/tween_test.lua`, run through `runtime.lua` | Pass |
| Field takeover, multiple callbacks, zero/negative duration | Targeted runtime probes | Pass |
| Invalid property value, easing, dt, delay, callback arguments | CLI `invalid` | Five expected type errors |
| Missing target property | Runtime probe | Expected explicit error on first update |
| Project runtime replacement | Built and ran `replacement` | Pass, project implementation returns x=99 |
| Cancellation during callbacks | Three runtime probes | TWEEN-01 |
| Shorthand after duration validation | Built and ran `after_invalid` | TWEEN-02 |
| Hover, completion, callback signature, diagnostics | Actual stdio LSP `probe.js`, `lsp.json` | Typed hover and setter completions; clean positive diagnostics; after signature exposes TWEEN-02 |
| Delay overshoot | Runtime probe | A 0.25 delay followed by update(1) leaves x=0; inherited discrete flux semantics, not counted as defect |
| Nested update reentrancy, long-running stress, visual LOVE integration | Not exercised | Coverage gap |

## TWEEN-01 — Cancelling tweens inside callbacks can crash the update loop

- **Severity/category:** medium, confirmed defect; inherited bundled flux behavior, not claimed newly introduced.
- **Reproduction:** `runtime.lua` probes `stop_from_onupdate`, `stop_other_from_oncomplete`, `stop_from_onstart`; raw `runtime.log`.
- **Expected:** documented `stop()` removal should leave the group usable and stop further mutation/completion of the removed tween. No callback cancellation restriction is documented.
- **Actual:** cancelling self in onupdate on its completion frame raises `tuatween.lua:253: attempt to index a nil value`; cancelling another tween in oncomplete raises `tuatween.lua:202: attempt to index a nil value`. Cancelling self in onstart still advances x from 0 to 0.75.
- **Source:** `vendor/runtime/tuatween.lua:199-223` iterates live array indices and continues processing after callbacks; `:249-259` removal compacts the array. A callback changes the collection while iteration/removal still assumes the original index is valid.
- **Blind spot:** `vendor/tests/tween_test.lua` covers cancellation outside update, and callbacks separately, without combining them.
- **Smallest maintainable improvement:** give update a stable iteration/removal contract and verify membership after callbacks; identity-based completion removal must not remove a different tween after array changes. Document this intentional deviation if retaining the current claim of unchanged upstream behavior is no longer accurate.
- **Regression:** cancel self in onstart/onupdate and another tween in oncomplete; assert no exception, no post-cancellation writes/completion, unaffected tweens advance once.
- **Known status:** no matching entry found in ROADMAP or inspected prior audit summaries. Upstream-derived implementation is explicit; compatibility choice should be acknowledged.

## TWEEN-02 — `after` shorthand accepts a nonnumeric duration

- **Severity/category:** medium, confirmed defect.
- **Reproduction:** `after_invalid/src/main.tua`: `local t = tween.to(box, 1, { x = 1 }); t:after("bad", { x = 2 })`. Run build then emitted Lua.
- **Expected:** reject string duration, just as `tween.to` rejects it; the documented shorthand is `after(seconds, values)`.
- **Actual:** build succeeds (`built 2 file(s)`, exit 0), then runtime immediately fails at `tuatween.lua:143: attempt to compare number with string`. LSP signature help labels the first shorthand argument `target: any` rather than seconds.
- **Source:** `vendor/runtime/tuatween.lua:52` folds two overloads into one signature with first argument `any`; runtime `:170-177` dispatches by argument count and treats the two-argument first value as duration.
- **Blind spot:** CLI `builtin_tween_checks_bundles_and_yields_to_a_project_runtime` and runtime suite do not validate a wrong shorthand duration; ordinary positive shorthand works.
- **Smallest maintainable improvement:** express and resolve distinct two- and three-argument callable signatures through shared LuaDoc overload infrastructure, keeping compiler and signature help aligned.
- **Regression:** accept both documented overloads; reject nonnumeric durations and invalid property values in each; signature help should show seconds first for shorthand.
- **Known status:** not found in ROADMAP or inspected prior audit summaries. This is distinct from the explicitly documented loss of type checking after later setter links; reproduction calls `after` directly on a typed local.

## Limitations and follow-ups

Callback annotations correctly reject a callback requiring an argument, since flux invokes callbacks with none. The common shortened-callback arity issue is not independently reproduced by this API's zero-argument callbacks and is not counted here; see `FOLLOWUPS.md`. The documented dynamic type after chained setters is a known limitation rather than a new finding. Arbitrary typed target/property correlation is not promised by current annotations (`target: any`), so misspelled target properties remain a runtime concern.
