# Functions and syntax lowering audit

Bounded public-interface audit of candidate 0.6.0. Three confirmed medium defects; shortened callback arity is a duplicate of STORES-01 and not counted again. No product edits. Applied tua-audit-lead, tua-language-auditor, and tua-semantic-resolver guidance.

## Reproduction and coverage

From repository root, run `python3 internal/audits/features-2026-11-07/functions/probe.py` and `node internal/audits/features-2026-11-07/functions/lsp.js`. The Python harness creates isolated configured projects, invokes the absolute existing `target/release/tua build`, and runs successful main.lua outputs with `/opt/homebrew/bin/lua` (5.4.8). All exact inputs, emitted Lua, build.log files and results.json are retained. No rebuild/full test suite was run.

| Area | Status / evidence |
| --- | --- |
| Defaults, optional parameters | Executed omitted defaults and explicit zero; optional omitted/nil/zero prints 7/7/0. Defects below. |
| Varargs and multiple returns | Executed typed vararg forwarding, annotated pair forwarding and destructuring. Wrong typed vararg tail rejected. |
| Closures and recursion | Executed recursive factorial and captured mutable counter with anonymous default parameter. |
| Runtime is / enums | Primitive string check lowers to type comparison; enum erases; invalid enum assignment rejected. Class/metatable is variants not exercised. |
| Compound assignments | Executed captured-local += and grouped *= (result 14); computed index correctly rejected. Field-chain/metatable side effects not freshly exercised. |
| Standard library | Executed table.insert, string.format, tonumber, math.floor, select, tostring and print. Output ok:3 / 12 / 2 / 3. Full catalog/overloads not audited. |
| Readable output | Inspected positive/build/main.lua: annotations/enums erased, simple default guards, original control structure preserved, no unnecessary helper runtime. Positive runtime: 120 / 3 / 5 / 3 / ok / 4 / 5 / 14 / yes / fast. |
| Editor | Actual stdio LSP pull and push diagnostics agree with CLI for four focused cases (lsp.json). Hover/completion/signatures not covered. |
| Sprite followup | Independently built copied exact return/local fixtures and asset. Return accepted; local rejected with TL1003. Runtime crash remains separately demonstrated by sprite agent; not rerun here. |
| Limits | LuaJIT/actual LOVE runtime not run in this pass; compiler final Lua 5.1 parser gate exercised through builds. Non-default config, parser recovery, deep recursion and performance not covered. |

## FUNCTIONS-01 — Return expressions skip call validation without matching return slots

**Medium · confirmed defect · new shared root (sprite followup).** `bad_return/src/main.tua` declares `takes(n: number): number` then returns `takes("bad")` at module scope. Build succeeds and actual LSP reports no diagnostics. `bad_local` assigns that identical call to a local: CLI and LSP report TL1003. Both explicitly annotated and inferred function-return controls reject the bad call. `bad_extra_return` returns `1, takes("bad")` from a function annotated `: number`: build succeeds and runtime prints `1 bad`. The issue is validation of the call itself, independent of whether surplus return values should be allowed.

The sprite return/local controls independently show the same distinction for a closed animation-name parameter: `sprite_return` builds, `sprite_local` reports expected `"idle"`, found `"absent"`. This consolidates the sprite report's uncounted followup.

Expected: validate every executed return expression's calls, even when no return contract applies or an expression has no corresponding declared result slot. Actual: invalid arguments bypass checks. Source: `crates/tua-core/src/check/mod.rs:1326` returns immediately for absent target, then line 1341 iterates only target slots; call validation occurs through inference at line 1381. Existing tests at `check/tests.rs:1530` exercise annotated multi-return values but do not cover module return argument errors or excess expressions.

Smallest maintainable improvement: traverse/check all return expressions once, independently of result-slot assignability; preserve Lua final-call expansion and avoid duplicate diagnostics. Add local/module-return/annotated-return/inferred-return/excess-return controls and LSP parity. Evidence: results.json, lsp.json, bad_return/, bad_local/, bad_extra_return/, sprite_return/, sprite_local/.

## FUNCTIONS-02 — Default expressions bypass their declared parameter type

**Medium · confirmed defect · new.** `bad_default/src/main.tua`:

```tua
local function f(x: number = "bad"): number return x end
print(f() + 1)
```

Build succeeds and actual LSP publishes/returns no diagnostics. Emitted Lua assigns `"bad"` under `if x == nil`; Lua exits 1 with `attempt to add a 'string' with a 'number'`. Expected a type mismatch at the default expression before emission, because the parameter/body/return contract asserts number.

Source: `crates/tua-core/src/parser/mod.rs:1258` stores defaults separately; `crates/tua-core/src/emit/mod.rs:2484` interpolates default expressions into guards. The checker has no default_parameters traversal (`rg -n default_parameters crates/tua-core/src/check/mod.rs` returns no matches). Existing `check/tests.rs:1550` and `:5190` cover compatible defaults/omission for named and anonymous functions, not incompatible default expressions.

Smallest maintainable improvement: check parsed default expressions in the function's real lexical/default evaluation scope against the parameter's post-default type, using shared expression inference. Do not add ad hoc literal-only checks. Add named/anonymous literal and call-result defaults, valid prior-parameter references, and CLI/LSP parity. Evidence: bad_default/, results.json, lsp.json.

## FUNCTIONS-03 — Explicit nil is rejected for a defaulted parameter

**Medium · confirmed defect · new.** `default_nil/src/main.tua` declares `f(x: number = 7)` and prints `f(), f(nil), f(0)`. CLI and LSP reject only the explicit nil with TL1003 expected number, found nil. Omission/zero control builds and prints 7/0. Optional-parameter control accepts nil. `docs/language.md` promises that defaults apply when the argument is nil.

To verify emission independently, run the absolute binary's `build --emit-on-error` inside default_nil, then `/opt/homebrew/bin/lua main.lua` inside its build directory. Preserved default-nil-runtime.json records runtime exit 0 and output 7/7/0. Expected the ordinary build to accept this documented nil-default workflow.

Source: `crates/tua-core/src/types/mod.rs:287` applies default/optional arity metadata but keeps parameter type; `crates/tua-core/src/check/mod.rs:2781` checks supplied arguments directly against that type. Default runtime guard is `emit/mod.rs:2484`. Existing default tests exercise omitted and numeric explicit arguments, not explicit nil. Retain separate call-site acceptance and post-default body type: allowing nil at calls must not weaken the body's guaranteed number. Add named/anonymous, aliases, and optional-versus-default controls. Evidence: default_nil/, default_omitted/, optional_nil/, results.json, lsp.json, default-nil-runtime.json.

## Duplicates and non-findings

- callback_short reproduces STORES-01's shared callback assignability issue with ordinary typed functions: a one-parameter callback for `(number, string) -> nil` is rejected with “callback requires 1 parameters, but it may be called with only 2”. Retain as non-domain regression coverage, not another defect count.
- default_scope shows `local x = 9; local function f(x: number = x)` returning nil. The emitted self-reference is visible; default name-resolution policy is not explicit enough to claim the outer binding should win. Record as a design/test question accompanying default checking, not a separate confirmed bug.
- ROADMAP.md and prior audit Markdown searches found optional/default signature-display work and prior contextual callback fixes, but no matching default-validation/nil-acceptance or top-level-return issue. FUNCTIONS-01 was already queued as this audit's sprite followup. No implication that all function or Lua semantics have been verified.
