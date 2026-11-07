# Object pools audit

Candidate `tua 0.6.0` at the shared baseline; product/release files preserved.
Oracle: Lua 5.4.8. Three Medium findings, one shared with the stores lane; no High
findings. No product changes or commits.

The explicit typed-factory workflow is usable: acquire/release item types survive
module imports, emitted Lua has correct ordinary LIFO/count/ownership behavior,
and existing exception/allocation tests pass. Lifecycle hook contracts and hook
reentrancy need attention.

## Reproduction commands

Run from repository root:

```sh
python3 internal/audits/features-2026-11-07/pools/probe.py
node internal/audits/features-2026-11-07/pools/lsp.js
lua internal/audits/features-2026-11-07/pools/runtime.lua "$PWD"
lua crates/tua-cli/tests/fixtures/object_pool_runtime.lua "$PWD"
```

CLI evidence is in `cli-results.json`; actual stdio LSP responses are in
`lsp-results.json`; runtime edge output is in `runtime-results.txt`.
`probe.py` creates separate projects for every case and records exact commands,
working directories, stdout, stderr and exit status. The existing runtime fixture
exits zero silently; `existing-runtime.log` is consequently empty.

## POOLS-01 — Typed factory does not constrain reset or dispose callbacks

**Confirmed defect · Medium · high confidence · CLI/build/runtime/LSP verified.**

Minimal source (`bad_reset/src/main.tua`):

```tua
type Bullet = { x: number }
local pool = objectPool({
  create = function(): Bullet return {x=0} end,
  reset = function(b: {name:string}) print(string.upper(b.name)) end,
})
local b = pool:acquire()
pool:release(b)
```

Expected: reject the incompatible hook argument contract. `create` establishes
that the hook receives a Bullet, not a record containing `name`. Actual: check and
build both succeed; emitted Lua fails at `string.upper(nil)` on release. The same
mismatch in `dispose` succeeds statically and crashes when clearing. As a negative
control, directly releasing `{name="wrong"}` is correctly rejected. As a positive
control, matching annotated hooks check, build and run successfully.

Actual LSP hover on an unannotated reset parameter is `any`, while acquired locals
hover as `Bullet`, field completion contains `x`, and release signature requires
`Bullet`; the item fact exists but is not propagated to construction hooks.

Likely root: `crates/tua-core/src/object_pool.rs:48–63` models reset/dispose as
`(any)->nil`. `pool_type_from_options` at line 81 extracts the create return only
for the returned pool. Improve shared contextual option inference so a proven
factory result supplies reset/dispose argument contracts without weakening `any`
for intentionally unknown factories. Add matched/mismatched annotated hooks,
unannotated hook completion/hover and imported factory cases. Existing checker
coverage at `check/tests.rs:286` uses explicitly matching hooks; the wrong-release
test at line 317 does not exercise hook compatibility.

## POOLS-02 — Reentrant reset corrupts ownership counts and free stack

**Confirmed defect · Medium · high confidence · Lua runtime verified.**

`runtime.lua` uses a reset hook that performs one guarded nested release of the
same object. This is finite (not an infinite recursion test). Both releases
return true; counts become `active=-1, available=2, capacity=1`. Two subsequent
acquires return the *same object* simultaneously. A second probe clears the pool
from reset: the outer release resurrects ownership with `active=-1, available=1,
capacity=0`. Lead independently reproduced both cases in
`../cross-checks/pools-runtime.txt`.

Expected: either reject reentrant lifecycle operations explicitly or preserve
`active + available == capacity` and never check out one identity twice. The
public guide promises duplicate releases do not corrupt the available stack
(`docs/object-pooling.md`, lifecycle and ownership sections). It does not document
reentrancy restrictions. This report does not demand nested release success; an
explicit error with consistent state is sufficient.

Root: `vendor/runtime/tuaobjectpool.lua:44–57` leaves state active while invoking
user reset, then commits unconditionally after the callback. Use an explicit
reset-in-progress state and a clear generation/operation guard, preserving the
existing guarantee that a throwing reset leaves the object active. Add bounded
nested-release and reset-calls-clear regressions, asserting counts, ownership and
unique simultaneous acquisitions. Existing runtime tests cover sequential double
release and reset exceptions but not reentrancy.

## POOLS-03 — Valid zero-argument reset callback is rejected

**Confirmed defect · Medium · high confidence · shared callback-arity defect.**

`zero_hook/src/main.tua` uses `reset=function() print("reset") end`. Expected:
accept a Lua callback that ignores its supplied object. Actual: TL1003
`callback requires 0 parameters, but it may be called with only 1`. The existing
runtime fixture itself uses a zero-argument reset for exception testing and passes.

Root: `crates/tua-core/src/types/assignability.rs:352–367` rejects absent source
callback parameters instead of allowing unused target arguments. This is the
same family as STORES-01; consolidate rather than implement a pool-specific
exception. Correct the diagnostic's reversed arity description and add zero-
and one-parameter hook tests plus a too-many-required-parameters negative control.

## Options, risks and improvement opportunities

- **POOLS-G01: finite prewarm validation.** `prewarm(math.huge)` passes the numeric
  validator (`tuaobjectpool.lua:61`) and continues creating values. The bounded
  probe forces its factory to fail at call five to avoid an actual allocation
  runaway. Add a finite-count guard; document capacity policy. This is an invalid
  numeric-input robustness risk, not proof that ordinary finite workloads hang.
- **POOLS-G02: optional capacity/trim policy.** The pool deliberately retains peak
  capacity and grows whenever empty; no maximum, exhaustion response, or trim
  method exists. The docs describe this accurately. A configurable bound or trim
  can help bursty games, but requires a nullable acquisition/overflow design.
- **POOLS-G03: disposal failure policy.** A throwing dispose aborts `clear` before
  ownership resets. Previously disposed objects remain tracked. Define whether
  cleanup continues and aggregates errors or deliberately permits retry; add
  exception behavior to the guide. Not counted as a bug without a stated policy.
- Active iteration is intentionally absent: the guide explicitly directs ECS to
  own populations. Do not add an iterator merely to duplicate active ownership.
- ROADMAP currently asks for representative LOVE pool benchmarks; this audit's
  existing 10,000-cycle allocation fixture is not such a benchmark.

## Coverage and limits

| Area | Status | Evidence |
|---|---|---|
| Typed construction, acquire/release, growth, prewarm, clear | Tested | positive project and existing runtime fixture |
| Sequential duplicate/foreign release, LIFO reuse, counters | Tested | positive project; object_pool_runtime.lua |
| Reset exception and duplicate factory identity | Tested | existing runtime fixture; runtime.lua control |
| Reentrant release and clear from reset | Tested | runtime-results.txt |
| Incorrect release payload and hook contracts | Tested | CLI per-case results |
| Factory-to-item typing across module import | Tested at protocol level | importedAcquired hover is Bullet |
| Completion, release signature, diagnostics | Tested at protocol level | lsp-results.json |
| Pool explorer backend | Tested | graph response includes pool declarations and operations |
| GUI explorer rendering, LuaJIT/LOVE population benchmark | Not exercised | no graphical/runtime qualification claim |
| Runtime override precedence | Not exercised | unrelated to confirmed issues |
| Unannotated factory useful item types | Documented limitation | intentionally any, not classified as defect |
