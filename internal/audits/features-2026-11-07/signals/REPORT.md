# Signals/events feature audit

Candidate: Tua 0.6.0, repository baseline in `../BASELINE.md`. Audit only; no
product files changed. **Three confirmed Medium defects**, two product
improvement opportunities. None was identified in the root roadmap's broad
signal-graph feedback item or prior devex finding index; this is not proof that
no historical report mentions them.

## Assessment and coverage

The normal event lifecycle is usable, imported schemas catch wrong emitted
payloads, and actual LSP queries recover handler payload types and field
completion. Important gaps remain in reentrant `once`, handler argument
validation, and navigation identity for a directly returned module bus.

| Area | Status | Evidence |
|---|---|---|
| Constructor lowering / bundled runtime | Tested | `positive`, check/build and emitted Lua run |
| Normal on/off/once, duplicates, listener snapshots | Tested | `runtime-controls.lua`, `runtime-controls.txt` |
| Mutation during emit, error recovery, allocation control | Tested | Existing `crates/tua-cli/tests/fixtures/signal_runtime.lua` exited 0 under Lua 5.4.8 |
| Earlier listener recursively emitting before once | Tested, defect | `once_nested`, `love-evidence.txt` |
| Pattern emission, nil payload preservation | Tested | Actual LOVE/LuaJIT `runtime_love` |
| Pattern subscription future-name behavior | Inspected | Documented intentionally current-name-only; runtime uses `_signals` keys |
| Explicit/imported schema checking | Tested | `imported`, expected TL1003 at bad score payload |
| Inferred unannotated handler parameters | Tested via LSP | `lsp-evidence.json`: `{ hp: number }` and `number`, `hp` completion |
| Event completion / emit signature | Tested via LSP | Imported `score` suggested; signature includes numeric payload |
| Event definition / references across direct export | Tested, defect | Same-file self-target after idle and after opening dependency |
| Handler argument validation | Tested, defect | `invalid_handlers` compiles then errors |
| Returned handler / listeners type precision | Tested via LSP | `any` and `{ }` respectively |
| Project runtime override | Tested | `override-evidence.txt`: output byte-identical to project runtime |
| Signal Graph UI, rename application, long-session throughput | Not exercised | Actual protocol navigation queried; no UI or edit-application claim |
| LuaJIT / LOVE | Tested | LOVE Lua 5.1 / LuaJIT 2.1.1700008891, headless fixture |

## Reproduce

From repository root:

```sh
ROOT="$PWD"
AUDIT="$ROOT/internal/audits/features-2026-11-07/signals"
(cd "$AUDIT/positive" && "$ROOT/target/release/tua" check && "$ROOT/target/release/tua" build && cd build && lua main.lua)
(cd "$AUDIT/once_nested" && "$ROOT/target/release/tua" build && cd build && lua main.lua)
(cd "$AUDIT/invalid_handlers" && "$ROOT/target/release/tua" check && "$ROOT/target/release/tua" build && cd build && lua main.lua)
(cd "$AUDIT/imported" && "$ROOT/target/release/tua" check)
lua "$AUDIT/runtime-controls.lua" "$ROOT"
lua crates/tua-cli/tests/fixtures/signal_runtime.lua "$ROOT"
python3 "$AUDIT/lsp_probe.py"
/Applications/LOVE.app/Contents/MacOS/love "$AUDIT/runtime_love"
```

Negative probes intentionally exit nonzero at their documented failing stage.
`cli-evidence.txt`, `love-evidence.txt`, `lsp-evidence.json` and controls preserve
outputs. LOVE fixture embeds this checkout's runtime directory; regenerate its
package.path if moving the repository. The CLI fixture emits ordinary Lua and
can be reproduced without LOVE; LOVE independently confirms supported-target
behavior.

## SIGNALS-01 — `once` can invoke twice during nested emits

**Medium · confirmed runtime defect · high confidence.** A one-time reward,
scene transition or cleanup can happen twice when an earlier handler emits the
same event recursively.

`once_nested/src/main.tua` registers a normal handler first. That handler performs
one nested `emit("hit")`. A `once("hit", ...)` listener is registered second.
Expected `count == 1`; emitted Lua reports `once count 2`. Actual LOVE LuaJIT
also reports `once_nested 2`. Lead independently reproduced count 2 and the
assertion failure; see `../cross-checks/first-domains.json`. Normal once and self-reentrant once controls pass.
This follows the documented statement that once removes itself before its first
call, and the intended at-most-once semantics of the API.

Observed source: `vendor/runtime/tuasignal.lua:54-64` removes the wrapper but has
no fired flag. `emit` retains an immutable handler-list version at lines 87-95.
The nested emit invokes/removes the wrapper; the outer version still contains
it and invokes it again. Copy-on-write is correct for ordinary subscriptions;
once needs its own invocation guard.

Existing runtime fixture tests a once handler that recursively emits **inside
itself**, after removal. It misses an earlier listener causing the recursive
emit. Smallest fix: a closure-local fired flag set before removal/callback,
leaving normal versioned dispatch untouched. Regression: both nesting orders,
normal two emits, cancellation before invocation, and throwing once callbacks.

## SIGNALS-02 — Statically invalid handlers compile and fail at runtime

**Medium · confirmed checking defect · high confidence.** `invalid_handlers`
contains `events:on("bad", 123)`, `once("bad", false)` and
`declare("bad", "not a schema")`. Check and build exit 0. Running emitted output
fails at the first registration with `signal handler must be a function`.

Expected: reject statically known non-functions while retaining explicitly
`any` dynamic values. The public docs require functions and runtime enforces
this in `vendor/runtime/tuasignal.lua:12-14,44-46,54-56,67-75`.

Likely shared root: `crates/tua-core/src/signal.rs:80-100` models the handler
parameter and return as `Any` for registration/declaration methods. This also
loses useful callable type information: actual hover on the result of `on`
shows `local handler: any`; `listeners` returns `{ }` (lines 193-210).
Existing checker tests around `check/tests.rs:653-735` validate emitted payload
schemas and permissive untyped **function** handlers, not scalar handlers.

Smallest fix: shared callable constraint for registration arguments, preserving
arbitrary valid callback signatures and explicit-any escape hatches. Preserve
the provided function type as the return type where feasible; `once` returns a
wrapper with the same parameter contract. Add CLI checker rejection cases plus
LSP signature/hover tests. Do not replace `Any` with an overly restrictive
zero-argument function that rejects legitimate callbacks.

## SIGNALS-03 — Directly exported bus loses definition/reference identity

**Medium · confirmed editor defect · high confidence; root cause inferred.**
`editor/src/events.tua` creates a local bus, declares numeric `score`, and returns
the bus. `editor/src/main.tua` requires it and emits `score`.

At zero-based `(line=5, character=15)` in main, definition should navigate to
`events.tua`'s `declare("score", ...)`; references with includeDeclaration should
include that declaration and the emit. Actual definition returns the emit's own
range; references contains only the emit. The result persists after a one-second
idle and explicitly opening the dependency. Meanwhile emit signature help does
show the imported numeric contract, so checking/signatures and navigation
disagree. The documented direct-return sharing example should not require
rewriting the bus as a table property just for navigation.

Likely root: `crates/tua-core/src/usage_index.rs:403-455` maps imported default
buses to their module path with `root_definition: None`, while the original
local bus retains its declaration span. Named property exports receive a symbol
span at lines 426-439; default exports do not. `signal.rs:450-468` groups by this
identity before ranking declarations, so the declaration is outside the group.
Existing LSP navigation coverage at `server/tests/navigation.rs:163` uses one
file/two local buses, missing direct-return module provenance.

Smallest fix: normalize default module-return bus identity to its underlying
local constructor/declaration using shared module provenance. Test direct return,
named export, re-export, unrelated same-name buses, unsaved dependency updates,
and definition/reference/rename consistency. Rename was not executed here;
shared usage indexing makes incomplete rename a credible related risk, not an
additional confirmed observation.

## Missing options and improvements (not defects)

- **SIGNALS-G01 — Scoped subscriptions / cleanup ownership.** `on` returns the
  handler, so scene teardown needs bus+event+handler bookkeeping or `clear()` of
  the entire bus. A lightweight subscription/disposer or owned scope would make
  selective cleanup easier. Keep existing `on` return compatibility. Runtime/API
  tables were inspected; no such option is currently exposed. Proposed acceptance:
  scope disposal removes only owned listeners and is idempotent during emission.
- **SIGNALS-G02 — Explicit pattern dispatch contract.** Current-name-only patterns
  are documented and not a bug. Cross-event dispatch order and mutation of event
  names during pattern emission use Lua `pairs` traversal (`tuasignal.lua:36-42`),
  without an explicit ordering guarantee. Document this boundary first; consider
  a snapshot-of-names option only if real game workflows require predictable
  batch dispatch. No deterministic mutation failure is claimed.

## Priority

Fix SIGNALS-01 first (runtime gameplay correctness), then shared callable
metadata (SIGNALS-02), then module bus identity (SIGNALS-03). The latter should
be coordinated with other feature agents' module/navigation findings. Scoped
cleanup is an API-design opportunity after correctness; do not silently change
ordinary emit snapshot semantics to fix once.
