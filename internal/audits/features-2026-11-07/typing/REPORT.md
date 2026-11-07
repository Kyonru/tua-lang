# Typing and inference audit

Bounded audit of release 0.6.0 at the supplied baseline. No product edits, builds of the compiler, commits, or extra agents. Four confirmed defects: **two High and two Medium**. Two Medium findings consolidate existing feature-lane observations; the missing-slot High is a residual path related to the previous typing audit. The two known nominal-assignment release failures are excluded.

## Reproduction and qualification

From the repository root:

```sh
python3 internal/audits/features-2026-11-07/typing/probe.py
node internal/audits/features-2026-11-07/typing/lsp.js
/opt/homebrew/bin/lua internal/audits/features-2026-11-07/typing/runtime-oracles.lua
```

Each CLI case is an isolated project at `cases/NAME`, with source `src/main.tua`, `tua.toml`, and (for runtime cases) emitted `build/main.lua`. The runner uses the absolute repository `target/release/tua`; individual reproduction is `cd cases/NAME` then invoke that absolute binary with `check` or `build`. `results.json` records command outputs and exit statuses; `lsp-results.json` records actual stdio LSP pull diagnostics and hover. Lua runtime is `/opt/homebrew/bin/lua` 5.4.8. Rules are disabled to isolate type checking; these are not missing-lint findings. Probe runners are observational and their exit status is not a claim that all cases pass.

The positive workflow checks/builds/runs typed records, arrays, insertion, dictionary guards, defaults, optional parameters, unions, gradual `any`, and local shadowing. Output: `4`, `Mina 3`, `ok!`, `6`, `dynamic`, `shadow`. Typed callback full-arity control passes; extra-required and wrong-type controls fail. Prior callback-return bug is rejected in this binary (`callback_return`). Stable assignment, array writes/insertion/removal and union rejection behave as expected in exercised cases.

## TYPING-01 — Dictionary dot writes bypass the value contract

**Confirmed defect · High · CLI/build/runtime/LSP verified · new in this audit.**

`cases/dictionary_runtime/src/main.tua`:

```tua
local scores: {[string]: number} = {}
scores.missing="wrong"
local n = scores.missing
if n then print(n + 1) end
```

Expected: reject the string write against the dictionary's number value contract. Actual: check/build exit 0, LSP has no diagnostics and hovers `n: number` inside the guard. Emitted Lua exits 1: `attempt to add a 'string' with a 'number'`. Lead independently reproduced the check acceptance and emitted-Lua crash in `../cross-checks/dictionary-assignment.json`. Both `scores["missing"]="wrong"` and a dynamic string-key write correctly fail with TL1003. Valid dot writes and dictionary deletion also pass.

Cause: `crates/tua-core/src/check/mod.rs:3301` gives plain field chains a dedicated assignment route; `resolve_direct_table_field_type` at `:3360` only handles Table and Union, returning None for Dictionary (`:3375`). `check_assignment` at `:828` skips assignability if the target is absent. Shared semantic read resolution handles Dictionary at `semantic.rs:2413`, but is not the write contract here.

Test blind spot: `check/tests.rs:153` exercises dictionary reads and guarded indexed writes; `:1389` uses a valid numeric dot write. Neither detects invalid dot mutation. Smallest maintainable improvement: resolve dictionary write keys/value contracts in the shared assignment-target resolver, consistently across dot and indexed syntax, preserving nil deletion and metatable write semantics. Regression: wrong dot/bracket/dynamic writes all reject, valid numeric/nil writes pass; later guarded hover and runtime agree.

## TYPING-02 — Reassignment ignores missing RHS slots

**Confirmed defect · High · CLI/build/runtime/LSP verified · residual related to prior TYP-05, not the known nominal failures.**

`cases/multi_assignment/src/main.tua`:

```tua
local n: number = 1
local s: string = "s"
n,s = 2
print(s .. "!")
```

Expected: reject nil assigned to required string `s`, per documented Lua assignment padding. Actual: check/build exit 0, LSP diagnostics empty and hover `s: string`; emitted Lua exits 1 with `attempt to concatenate a nil value (local 's')`.

Cause: `check/mod.rs:817` zips LHS variables with syntactic RHS expressions. It never visits `s`, rather than applying Lua value-list expansion/padding. This also warrants final-call reassignment regressions, but the confirmed reproduction is the scalar missing slot. Smallest maintainable improvement: reuse shared value-list slot semantics already used for local initialization and call argument expansion; check every LHS against its actual slot without losing stable binding contracts.

Test blind spot: `check/tests.rs:5347` (`missing_scalar_assignment_slots_are_nil`) only declares new locals. Prior `internal/audits/devex-2026-11-06/typing/REPORT.md` TYP-05 identified absent declaration slots becoming any; those declarations now have tests, but existing-local reassignment still skips the slot entirely. Regression should cover scalar/parenthesized-call nil padding, expanded final multi-return calls, and member LHS targets. Count as a residual shared value-list issue in synthesis, not an unrelated discovery of Lua padding semantics.

## TYPING-03 — Short callbacks are rejected even when extra arguments are ignored

**Confirmed defect · Medium · CLI/LSP/runtime verified · consolidates STORES-01 and POOLS-03.**

`cases/callback_short/src/main.tua`:

```tua
local function use(cb: (x: number, y: string) -> nil) cb(1,"a") end
use(function(x) print(x+1) end)
```

Expected: accept a callback ignoring `y`, as ordinary Lua does and the store documentation demonstrates. Actual TL1003: `callback requires 1 parameters, but it may be called with only 2`. Explicitly annotating `x: number` does not help. Full-arity control passes; callbacks requiring too many arguments or incompatible argument types correctly fail. Ordinary-function callback `x` hovers as number, so the stores' `any` hover symptom should not automatically be treated as this same inference defect. Pure Lua oracle passes.

Cause: `types/assignability.rs:351` iterates target parameters and rejects a missing source parameter at `:358`, despite the earlier required-argument-count guard already rejecting callbacks requiring too many arguments. The error message describes the inverse safety condition on this branch (`check/type_mismatch.rs:89`). Fix shared function assignability, retaining contravariant type checking and extra-required-parameter rejection; do not add pool/store-specific exceptions.

Test blind spot: `check/tests.rs:1816` expressly accepts shortened LOVE callbacks on a separate path; store tests use complete callbacks. Regression matrix: zero/one/full parameters, optional/vararg callback tails, bad parameter types, too many required parameters, typed locals as well as direct calls. Reuse existing stores/pools fixtures as integration coverage. These are one consolidated defect, not three counts.

## TYPING-04 — Optional record fields cannot be cleared with nil

**Confirmed defect · Medium · CLI/LSP/runtime verified · consolidates STORES-02.**

`cases/optional_dot/src/main.tua`:

```tua
type Row = {value?: number}
local row: Row = {}
row.value = nil
```

Expected: remove an optional field; Lua nil assignment makes it absent. Actual TL1003 `expected number, found nil`, in CLI and LSP. Literal-bracket form also fails. Explicit `value: number | nil` accepts the write; required `value: number` correctly rejects nil. Pure Lua deletion oracle passes. `optional_init_nil` also rejects `{value=nil}`; this is related optional representation behavior, not an extra finding.

Cause for dot writes: `check/mod.rs:3362` returns `field.ty` and drops `TableField.optional`. Preserve optionality when deriving writable field contracts. Treat read and write contracts deliberately rather than patching the store callback. Test blind spot: `types/tests.rs:509–530` verifies optional shape compatibility, and store optional tests cover reads, not deletion. Regression: plain/nested records, store update/get writes, bracket spelling, nil constructor field, required-field negative control and incompatible non-nil values.

## Coverage and limits

| Area | Status / evidence |
|---|---|
| Positive annotations, aliases, arrays/dictionaries, defaults, any, shadowing | Check/build/runtime; `positive`, `shadow` |
| Invalid stable assignments, unions, indexed writes, insert/remove, callback contracts | CLI positive/negative controls in `results.json` |
| Callback and record issue consolidation | Ordinary-type reproduction plus prior stores/pools reports; shared source inspected |
| CLI/LSP parity | Actual LSP for seven cases; `lsp-results.json`; no GUI claim |
| Record path narrowing/invalidation | `guard_positive` and `guard_invalidation`; runtime positive works, mutation invalidates access |
| Generics/native tuple boundaries | `generics` rejects TL0004 as documented; native positional annotation rejects syntax, also documented |
| LuaDoc positional tuples | Probed, limited: `lua_tuple` rejects `{1,"a"}` against Pair and indexed reads yield mixed-type errors; no usable tuple workflow demonstrated |
| Nominal classes/module contracts | Excluded; known release failures and other lanes own this scope |
| LuaJIT / LOVE / performance / recursive depth / incremental edits | Not exercised |

Additional observations are deliberately not promoted into confirmed findings within this bounded pass. `guard_positive` succeeds but warns that an optional field's nil comparison is always false; this warrants an optional-read follow-up. An inline nested record parameter spelling produced a parser failure; using an alias enabled the workflow, and parser causality was not isolated. LuaDoc tuple syntax is documented as supported but the straightforward mixed positional initializer/read workflow fails: inspect `semantic.rs:2433` (tuple indexing unions all element types) and tuple/array initializer assignability before claiming fixed-position checking. These are concrete limits of coverage, not passes.

ROADMAP.md has general inference/overload work and deferred generic classes; it does not document dictionary dot-write bypass or optional deletion as intended limitations. Native tuple/generic omissions are already documented capability boundaries, not defects. Prior typing audit and release baseline were consulted; no broad claim of regression introduction is made. Highest-value improvements are shared assignment-target contracts and consistent Lua RHS slot normalization, followed by shared callback arity and optional-field write handling.
