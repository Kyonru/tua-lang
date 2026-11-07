# State machines feature audit

Audit date: 2026-11-07. Candidate: `tua 0.6.0`, baseline revision and pending
metadata as recorded in `../BASELINE.md`. Product code was not changed.

## Assessment

Literal configurations provide useful exact state/event assistance and the
normal synchronous, wildcard, payload, async-resume and explicit-cancellation
paths work. Four medium-severity defects were reproduced, spanning model
construction and callback lifecycle. None was located in the current ROADMAP or
prior top-level findings searched; this means newly documented here, not proof
that nobody previously reported them. No high/critical findings are claimed. The lead independently reproduced SM-03
and SM-04; see `../cross-checks/first-domains.json`.

## Reproduction and evidence

Run commands from repository root. `TUA` below is a shell convenience for the
absolute `target/release/tua` path; `AUDIT` is the absolute directory containing
this report.

```sh
TUA="$PWD/target/release/tua"
AUDIT="$PWD/internal/audits/features-2026-11-07/state_machines"
(cd "$AUDIT/happy" && "$TUA" check && "$TUA" build && cd build && lua main.lua)
(cd "$AUDIT/reserved" && "$TUA" check && "$TUA" build && cd build && lua main.lua)
(cd "$AUDIT/dynamic" && "$TUA" check)
(cd "$AUDIT/edges" && "$TUA" check && "$TUA" build && cd build && lua main.lua)
python3 "$AUDIT/probe_lsp.py"
lua crates/tua-cli/tests/fixtures/state_machine_runtime.lua "$PWD"
lua "$AUDIT/runtime.lua" "$PWD" "$AUDIT/special.dot"
```

The reserved-name runtime and dynamic checking commands intentionally fail.
Lua runtime: Homebrew Lua 5.4.8. All generated Lua and runtime copies remain
under the fixture `build/` directories. Raw results: `results.json`,
`edge-build-results.json`, `runtime-results.txt`, `lsp-results.json`.
The original exploratory `results.json` runtime entry includes a harness error
(`tostring` called with zero returns); the corrected `runtime.lua` and
`runtime-results.txt` supersede that entry. All findings below use corrected
execution and, for callback findings, also emitted Tua execution in `edges/`.

| Area | Status | Evidence |
|---|---|---|
| Basic transitions, wildcard sources, `is` and multi-return `can` | Tested, passed | `happy/`, check/build/Lua |
| Payloads including nil holes, async leave/enter, explicit cancellation, allocation smoke | Tested, passed | Existing runtime suite; `happy/` |
| Reserved event names | Tested, defect | `reserved/`; LSP completion/hover |
| Partially dynamic initial state | Tested, defect | `dynamic/`; checker/hover/inspector |
| Before-callback cancellation sequencing | Tested, defect | `edges/`, `runtime.lua` |
| Nested transition from after callback | Tested, defect | `edges/`, `runtime.lua` |
| Exact state/event completion and graph metadata | Protocol tested | `lsp-results.json`; normal unions/graph agree |
| DOT output for spaces/wildcards | Output tested; renderer blocked | `special.dot`; Graphviz `dot` unavailable |
| Full graphical editor preview and navigation | Not exercised in this lane | Actual `tua/inspect` protocol checked only |
| LOVE/LuaJIT runtime compatibility | Not exercised in this lane | Runtime requires no LOVE; Lua 5.4 oracle only |
| Runtime override | Inspected only | Documentation/build code contract; no replacement probe |
| Configuration gaps | Inspected/design questions | Options schema and documented API |

## SM-01 — Event names can overwrite typed API fields without a diagnostic

- **Category/severity/confidence:** confirmed defect / Medium / high.
- **Impact:** a configuration accepted by checking and build exposes a method
  completion that immediately crashes. This is not a general dynamic-Lua escape:
  the event name and call are both literal/static.
- **Minimal input:** `reserved/src/main.tua`, six lines. Declare an event named
  `current`, then `local fired: boolean = machine:current()`.
- **Expected:** reject the collision with a helpful declaration diagnostic, or
  provide collision-free dispatch whose generated/runtime behavior agrees with
  the editor. Existing `current` is documented as a state value.
- **Actual:** `check` and `build` return zero; Lua raises `attempt to call a
  string value (method 'current')`. Actual LSP hover reports
  `function StateMachine:current(...: any): boolean` and completion offers it.
- **Root cause:** `crates/tua-core/src/state_machine.rs:486` blindly inserts event
  function fields over fixed machine members. Runtime
  `vendor/runtime/tuastatemachine.lua:159-168` initializes `fsm.current` first and
  keeps it through `fsm[name] = fsm[name] or create_transition(name)`.
- **Coverage gap:** core state-machine tests derive ordinary event names; runtime
  fixtures likewise avoid collisions. Neither checks agreement for fixed names.
- **Smallest fix direction:** centralize reserved instance/prototype members in
  the domain model and issue a static configuration error; runtime validation
  should fail clearly for dynamic collisions. Do not silently reinterpret a
  built-in method as an event.
- **Regression:** parameterized fixed-member collision cases (current, events,
  can, transition, constants), checking/hover and emitted runtime; retain normal
  event/state shorthand-name behavior.

## SM-02 — A present nonliteral initial state becomes a fictional `none`

- **Category/severity/confidence:** confirmed defect / Medium / high.
- **Impact:** valid configurations are rejected, and the graph/hover display an
  initial state that will never be selected by this configuration.
- **Reproduction:** `dynamic/src/main.tua`: `local start = "idle"`, direct options
  table with `initial = start`, wildcard reset to `running`, then
  `machine:is("idle")`.
- **Expected:** resolve the known local value where supported, or conservatively
  retain string/unknown state information. Docs `state-machines.md:126` default
  `none` only when `initial` is omitted; dynamic information is a gradual boundary.
- **Actual:** TL1003 says expected `"none" | "running"`, found `"idle"`.
  Hover on `machine.current` has the same wrong union; `tua/inspect` reports
  initial `none` and completion invents `onnone`, `onenternone`, `onleavenone`.
- **Root cause:** `crates/tua-core/src/state_machine.rs:1047-1050` conflates absent
  initial and present-but-not-string-literal expressions. Runtime line 160 uses
  the actual provided value. The inspector consumes the same static spec, so the
  inconsistency affects more than checking.
- **Coverage gap:** existing derivation tests use literal `initial` only.
- **Smallest fix direction:** distinguish absent, statically resolved and unknown
  initial values in shared domain facts; generic fallback must not invent names.
  Make the inspector explicitly incomplete when the initial state is unknown.
- **Regression:** absent initial still yields `none`; local/computed initial
  does not; checker, hover, completion and inspector agree. Also test partially
  dynamic event arrays/source/destination values.

## SM-03 — Cancelled before callback still executes leave side effects

- **Category/severity/confidence:** confirmed callback-contract defect / Medium /
  high for observation; expected sequencing follows Tua's documented timing.
- **Reproduction:** `edges/src/main.tua` first block (or `runtime.lua`):
  `onbeforego` returns false, `onleaveidle` increments a counter.
- **Expected:** returning false before the event starts prevents the leave phase
  (`docs/state-machines.md:137`). This matters when leave callbacks unsubscribe
  handlers, hide UI, stop audio, or destroy state resources.
- **Actual:** prints `cancel false idle leave callback count 1`: state unchanged,
  but its leave effects already ran. Both check/build succeed.
- **Root cause:** `vendor/runtime/tuastatemachine.lua:104-109` calls both callbacks
  before checking either cancellation return.
- **Coverage gap:** existing runtime cancellation test checks state and pending
  flags, but not whether the leave callback ran after before cancellation.
- **Fix direction:** short-circuit after `onbefore` false before invoking leave;
  preserve explicit leave cancellation and payload forwarding. This changes an
  observable inherited runtime quirk, so include a compatibility note rather
  than assuming upstream behavior is an intentional Tua contract.
- **Regression:** before false + leave side effect; assert zero leave/enter/after
  calls, unchanged state, cleared pending metadata, and ability to retry.

## SM-04 — Chaining an asynchronous transition from an after callback loses it

- **Category/severity/confidence:** confirmed defect / Medium / high.
- **Impact:** common gameplay sequencing (`onaftergo` starts a fade/deferred
  `finish`) silently cancels the new transition's bookkeeping. Completion code
  cannot resume it, leaving the state at `run`.
- **Reproduction:** second block in `edges/src/main.tua` / `runtime.lua`. `go`
  changes idle→run; `onaftergo` calls `finish`; `onleaverun` returns `self.ASYNC`.
- **Expected:** after the completed `go`, the new `finish` remains pending and
  `transition("finish")` reaches done. `onafter` is documented as running after
  the event completes (`state-machines.md:140`).
- **Actual:** `chain true run none nil`; resuming prints `nil run none`.
- **Root cause:** completion callbacks execute before unconditional reset of
  `asyncState` and `currentTransitioningEvent` at runtime lines 133-136. The
  nested transition starts correctly, then its pending state is overwritten by
  the old transition's completion cleanup.
- **Coverage gap:** existing tests cover starting another event externally while
  one is pending, but not starting it inside `onafter` or `onstatechange`.
- **Fix direction:** establish completion bookkeeping before user completion
  callbacks, or associate cleanup with a transition identity so it cannot clear
  a newer transition. Define reentrancy policy consistently for other phases.
- **Regression:** nested sync and async events from after/state-change callbacks,
  retry/resume/cancel, payload identity and final flags. Avoid only testing final
  state; assert the second transition's pending metadata before resumption.

## Missing options and improvement opportunities

1. **Explicit reentrancy/pending-event policy** (design question). Currently an
   external event can abandon a pending transition and even return true when
   its own route cannot run (existing runtime tests deliberately preserve it).
   Document this compatibility behavior and consider opt-in reject/queue/replace
   policies rather than silently changing existing games.
2. **Collision-free dispatch by name** (feature gap). A `fire(name, ...)` surface
   would let computed events and names overlapping machine API members be used
   safely. This needs a design decision, not merely more completion entries.
3. **DOT escaping** (observed output risk; no Graphviz oracle available). For
   names containing spaces and wildcard sources the writer emits
   `* -> game over [label=start game];` without quoted/escaped identifiers.
   Source: runtime `todot` around lines 193-196; artifact `special.dot`. Add a
   Graphviz-backed syntax test before recording this as independently validated
   renderer failure. SVG rendering was not claimed.
4. **Configuration diagnostics and partial-static metadata.** Unknown callback
   keys are allowed through `callbacks: any`; reserved names, duplicate routes,
   incomplete dynamic configurations and callback typos would benefit from
   precise domain diagnostics based on the same model as completion/graph data.

Recommended order: shared configuration identity/unknown-value handling (SM-01,
SM-02), callback lifecycle semantics (SM-03, SM-04), then explicitly designed
pending-event policies and dispatch APIs. Do not patch only hover or graph output.
