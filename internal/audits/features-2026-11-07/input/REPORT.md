# Input/Baton feature audit

Candidate `tua 0.6.0`, baseline `9d490dc`; audit only. Two confirmed **Medium** validation defects. Basic typed input, name completion, pair return inference, pointer tracking and override behavior are dependable in the exercised cases. No High finding.

## Reproduction and evidence

From repository root:

```sh
python3 internal/audits/features-2026-11-07/input/probe.py
python3 internal/audits/features-2026-11-07/input/editor-probe.py
lua internal/audits/features-2026-11-07/input/runtime.lua "$PWD/internal/audits/features-2026-11-07/input"
```

The first script creates isolated projects, checks/builds each with the absolute `target/release/tua`, and saves `cli-results.json`. `runtime.lua` executes the emitted positive and negative fixtures using **deterministic LOVE keyboard/mouse/joystick doubles under Lua 5.4.8**; this is not a physical-controller or actual LOVE test. Output is preserved in `runtime-results.txt`. `editor-results.json` contains actual stdio LSP positions/responses; no VS Code GUI claim. `override-results.json` records preservation and actual execution of a project-supplied `baton.lua`.

## INPUT-01 — Numeric spelling bypasses deadzone validation

**Confirmed defect · Medium · high confidence · checker/build/runtime verified.**

```tua
local c = input({controls={go={"key:a"}},deadzone=-0.25})
```

Expected: the same `TL5008: input deadzone must be between 0 and 1` as an out-of-range positive literal. Actual: check/build succeed. `deadzone=0x2` also succeeds, whereas `1.25` is rejected. Negative numbers are unary expressions, and hexadecimal literals cannot be parsed with Rust decimal `f64::parse`. `crates/tua-core/src/input.rs:1127–1141` only matches `Expression::Number` and silently ignores parse failure. Runtime effect: negative deadzone reports `kbm` with no held input (`vendor/runtime/baton.lua:164`); hexadecimal 2 prevents a fully held key from activating.

Minimal fixtures: `negative_deadzone/`, `negative_hex/`; rejected control: `large_deadzone/`. Existing `input::tests::validates_sources_and_pairs` tests decimal `1.5` only (`input.rs:1347`). Use the shared Lua numeric literal/constant parser, including unary signs, without evaluating arbitrary user expressions. Add negative decimal and hex boundary cases (valid 0/1 and invalid -0.25/0x2), and ensure genuinely computed values remain conservative. Runtime validation for computed invalid ranges is a separate defensive improvement.

## INPUT-02 — Statically empty controls are treated as unknown controls

**Confirmed defect · Medium · high confidence · checker/build/runtime verified.**

```tua
local c = input({controls={}})
print(c:get("missing"))
```

Expected from docs/input.md's static-name contract: reject an unknown static name. Actual: check/build succeed, then emitted Lua throws `No control with name "missing" defined`. The same call with `controls={go={"key:a"}}` is rejected as expected (`known_unknown/`). Empty input is useful for pointer-only players, so rejecting the constructor itself would be wrong.

Likely cause: `input.rs:324` infers exactness from nonempty names rather than whether the constructor config is statically known. `literal_union_or_string` (`input.rs:709`) widens an empty list to unrestricted string. Thus empty and dynamic configs share the generic fallback. Runtime correctly rejects the nonexistent control (`vendor/runtime/baton.lua:275–282`). Existing inference tests cover populated names and dynamic configuration, not known-empty name sets. Preserve a separate static/unknown flag in `InputSpec`; validate named calls against even an empty known set while retaining generic dynamic `Input`. Add direct and imported empty-input negative tests, pointer-only positive tests, and editor diagnostics consistency.

## Capability and usability improvements (not counted as confirmed defects)

- **INPUT-G01 — Literal-aware overload selection.** Actual signature help inside `c:get("move")`, after the complete `move` text, selects active signature 0: the scalar control overload. Both scalar and two-return pair signatures are present, and inferred x/y are correctly `number`. Shared selection uses argument count alone (`signature_help/mod.rs:1062–1075`), so same-arity name overloads cannot select the relevant one. Prefer compiler-proven literal/type ranking with arity fallback, shared by input and other overloaded APIs. Existing input signature test (`signature_help/tests.rs:512`) asserts available labels rather than active literal-selected signature. Reconcile with the editor lane; do not describe the whole signature response as missing.
- **INPUT-G02 — Configuration boundaries.** There is no per-control deadzone/response curve, repeat/hold timing, or named remap/context-stack API in the documented input surface. These are product options, not violations of Baton's polling contract. Prioritize a documented rebinding pattern before adding runtime machinery; keep keyboard-layout scancodes and gamepad selection explicit. Numeric hat sources above one digit (`hat:10u`) are rejected and unsupported by the bundled single-digit parser (`baton.lua:41`, `input.rs:1233–1236`); document that boundary or support multi-digit indices consistently.
- **INPUT-G03 — Pointer preconditions.** `pointer()` and `pointerDelta()` always appear in the type, even when a static config disables them; their hovers correctly explain `pointer=true`, and runtime explicitly errors. Optional static guidance could prevent this misuse without changing the runtime contract. Do not turn dynamic pointer configuration into a false-positive diagnostic.

## Coverage

| Area | Status / evidence |
|---|---|
| Source literals, control names, pair order references, wrong pointer type | Tested: cli-results.json (valid and invalid controls) |
| Scalar and two-return pair checking | Tested: positive project, typed x/y assignments |
| Keyboard press/down/release, pointer cache/deltas | Tested with deterministic LOVE doubles: runtime-results.txt |
| Joystick radial deadzone | Tested with deterministic axis double |
| Square deadzone | Inspected only: baton.lua:218–225; physical diagonal behavior not exercised |
| Dynamic config fallback | Checked/built successfully: dynamic/; runtime not run in this lane |
| Negative/hex deadzone; known-empty names | Tested: confirmed findings above |
| Custom runtime override | Build preserves source bytes; executed output `CUSTOM-BATON` |
| Actual LSP hover, pair inference, source/name completion, signature help | Tested: editor-results.json |
| Imported input identity, physical gamepads, actual LOVE/LuaJIT, GUI | Not exercised; no qualification claim |

No matching input-specific issue appears in the current ROADMAP.md or prior devex findings inspected. These are separate from the known nominal-assignment and inline-eval release blockers. No product code, release metadata, or commits changed.
