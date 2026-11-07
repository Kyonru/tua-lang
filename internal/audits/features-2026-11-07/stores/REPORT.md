# State stores feature audit

Baseline: candidate `tua 0.6.0`, repository `9d490dc`, pending release metadata preserved. Audit only; no product edits. Lua oracle: Homebrew Lua 5.4.8. LuaJIT/LOVE runtime execution and actual VS Code GUI not exercised in this lane.

## Assessment

The basic explicit-store workflow is usable: imported store creation, checked updates, replacement, keyed notifications, idempotent unsubscribe, runtime emission and existing notification/allocation regression fixture pass. Shortened inferred `watch` callbacks are incorrectly rejected, despite ordinary Lua accepting ignored extra arguments. Optional state-field deletion also encounters a broader checker defect. Reentrant replacement has a smaller documented-return mismatch.

## Reproduce

Run from repository root:

```sh
python3 internal/audits/features-2026-11-07/stores/check-probes.py
python3 internal/audits/features-2026-11-07/stores/editor-probe.py
lua internal/audits/features-2026-11-07/stores/runtime-probes.lua "$PWD"
lua crates/tua-cli/tests/fixtures/store_runtime.lua "$PWD"
TUA="$PWD/target/release/tua"
(cd internal/audits/features-2026-11-07/stores/project && "$TUA" check && "$TUA" build && cd build && lua main.lua)
```

`check-probes.py` preserves named source snippets and exact outputs in `typing-results.json`. `followup-results.json` includes smaller positive/negative controls; to run one independently, copy its `.tua` file to `typing/src/main.tua` and run the absolute repository `target/release/tua check` from `typing/`. `baseline-results.json` records successful end-to-end commands. LSP raw request/responses are in `editor-results.json`.

## Findings

### STORES-01 — Shortened inferred watch callbacks fail checking

**Confirmed defect · Medium · high confidence · CLI, runtime and LSP verified. Shared callback-assignability cause.**

```tua
local game = store({ score = 0 })
game:watch("score", function(value) print(value) end)
```

Expected: accepted; unused `previous` and `state` arguments may be ignored in Lua. The public store guide itself demonstrates a two-argument watcher (docs/stores.md:70). Actual: TL1003, `callback requires 1 parameters, but it may be called with only 3`. Zero- and two-parameter inferred callbacks fail likewise. Three-parameter equivalent passes (`three-parameter-control.log`). Lead independently reproduced the one-parameter failure in `../cross-checks/store-callback.json`. All four forms work in Lua (`runtime-results.log`). In actual LSP, shortened watcher `value` hovers as `any`; `update` state and its `score` completion remain typed, so the problem is not universally absent store metadata.

Likely cause: `crates/tua-core/src/types/assignability.rs:351` iterates every target callback parameter and rejects missing source parameters instead of allowing Lua functions to ignore extra values. The diagnostic at `check/type_mismatch.rs:89` describes the inverse count condition for this path. Store watch overload construction (`store.rs:164–220`) exposes this path. Existing `accepts_typed_store_state_updates_and_subscriptions` explicitly annotates all three watch parameters; runtime tests do not check callback typing. Improve shared callback assignability, retaining checks for a callback demanding *more* required arguments than supplied. Add zero/one/two/three argument watch tests plus malformed callback contracts and editor parameter hover regressions. Reconcile with other feature lanes before counting separate arity symptoms.

### STORES-02 — Optional state fields cannot be cleared with nil

**Confirmed defect · Medium · high confidence · CLI/runtime verified; shared with plain record assignment.**

```tua
type State = { score?: number }
local initial: State = {}
local game = store(initial)
game:update(function(state) state.score = nil end)
```

Expected: remove the optional field using Lua's `nil` assignment. Actual: TL1003 `expected number, found nil`. The equivalent plain typed record fails identically (`plain_optional_nil.tua`, `followup-results.json`), so this is not a store-specific regression. The runtime supports deletion and notifies watchers with nil (`runtime-probes.lua`). Optional watch callback values already become `number | nil` in `store.rs:175–180`, while the write check does not preserve optionality. Root-cause location requires the typing lane to resolve the shared field-assignment contract. Existing optional-store coverage (`check/tests.rs:394`) tests reading watcher values, not clearing fields. Add plain-record, update callback and `get().field = nil` tests, with required-field nil rejection as a control. No special store checker patch recommended.

### STORES-03 — Reentrant set returns a later state rather than its argument

**Confirmed documentation/runtime mismatch · Medium · high confidence · runtime verified.**

`runtime-probes.lua` installs a subscriber that calls `set(second)` when receiving `first`, then invokes `local result = game:set(first)`. Expected from docs/stores.md:57: result is `first` (“return next”). Actual: `result == second`; output is `REENTRANT_SET returned_next=false returned_current=true`. `vendor/runtime/tuastore.lua:108–110` notifies before returning the mutable current `_state`. Existing runtime coverage includes nested updates but does not assert nested `set` return identity. Either return `nextState` as documented or explicitly document “current state after all synchronous listeners.” Prefer deciding this as part of a consistent reentrancy contract, then add a nested-set return identity test. No claim that nested callback notification order itself violates a specified ordering guarantee.

## Missing options and design opportunities

- **STORES-G01: union-key watch selection.** A key typed `"score" | "paused"` is rejected even with a three-parameter callback (`watch_variable_three.tua`). Generated one-field overloads (`store.rs:173–214`) cannot represent dynamic selection over several fields. A shared overload-union policy or a constrained key-union overload could provide `number | boolean` callback values. This is a capability gap, not a claim that arbitrary string keys should be accepted.
- **STORES-G02: selectors/batching/patch convenience.** The API has five methods (`store.rs:13`); no derived selector, equality comparator, transaction/batch, or patch method. Consider selectors and explicit batching for UI/persistence adapters, with clear notification/error semantics; avoid making persistence format or migration policy global. Documentation already intentionally delegates serialization, debounce and migration to applications.
- **STORES-G03: callback failure/reentrancy policy.** Listener exceptions propagate and stop later listeners; state has already changed. Existing runtime test explicitly expects propagation. Document that subscriptions are synchronous and errors do not roll back mutations; decide separately whether optional guarded listeners are desirable.

## Coverage and limits

| Area | Status | Evidence |
|---|---|---|
| Imported store check/build/emitted Lua runtime | Tested | project/, baseline-results.json |
| set/update/watch/unsubscribe positive behavior | Tested | project/src/main.tua, runtime-probes.lua |
| Invalid state updates, replacements, callback types, watch keys | Tested | typing-results.json |
| Optional values/deletion | Tested | typing-results.json, followup-results.json, runtime-results.log |
| Shallow snapshots, persistence subscription adapter | Tested | runtime-probes.lua; actual filesystem serializer not supplied |
| Listener additions/removals, no-op updates, nested update scratch allocation | Tested | existing store_runtime.lua fixture, exit 0 |
| Reentrant set return identity | Tested | runtime-results.log |
| Direct-store hover/completion/signature/definition/diagnostics | Tested at LSP protocol | editor-results.json; no GUI claim |
| Direct call-result module property editor inference | Unresolved symptom | editor-import-results.json returned any/no signature; direct-store control worked. Needs module/editor lane cross-check before confirmed finding |
| Runtime override precedence | Inspected only | cli.rs:3296 `build_does_not_overwrite_project_tuastore_runtime` |
| LOVE persistence, LuaJIT oracle, graphical inspector | Not exercised | host runtime qualification remains separate |

No store-specific roadmap entry matched `store`/`subscription` in the current ROADMAP.md. These observations are not the previously reported nominal-alias assignment failures; STORES-01/02 should be reconciled with the broader typing lane. Counts before cross-feature consolidation: **three confirmed Medium findings**, zero High; three capability/design opportunities. Imported editor inference remains an unconfirmed observation, excluded from the count.

## Final reconciliation

The initial import observation below was subsequently isolated and confirmed as MODULES-01; see [module audit](../modules/REPORT.md). Short callback and optional-nil findings are consolidated under TYPING-03/04.
