# M7 authored waits and suspension

Implemented on `feature/tdialogue-rich-text`; not merged or published. This slice
adds duration waits and suspension to the previous action-handle foundation.
Voice, speech profiles, audio adapters and non-cancellable skip policy remain open.

## Contracts

`@wait seconds [target:label]` occupies an entire label. Durations are finite and
between 0 and 86,400 seconds. Parsing validates exclusivity and target existence;
emission records source coordinates and selects schema 9 only for wait documents.
All older schema selections remain unchanged. Schema 9 composes existing rich text,
context, events, expressions and actions. Older replacement runtimes must implement
schema 9 to accept wait documents.

A player in `waiting` exposes an owned wait snapshot, not a line presentation.
Only host-supplied dialogue time advances it. One update consumes at most one wait
and discards excess time, including for zero-duration loops. Automatic branch
traversal still uses the existing transition budget. Presentation failure faults
once; subsequent updates do not retry side effects or resolution.

Suspension is a flag, not a replacement state. It preserves execution identity,
clocks, choice presentation and action ownership. Updates freeze; navigation and
action dispatch/consumption return false. Resume never dispatches actions or catches up wall time.
Context refresh cannot
navigate a suspended choice; resume reconciles any deferred empty-choice fallback
with the existing transition budget. A thrown resolver error preserves suspension
and the previous visit for explicit retry. External work may settle while suspended. A synchronous
handler that suspends before returning gets its boolean result queued in an owned
handle rather than navigating or being called again. Lifecycle reset/cancellation/
disposal clears suspension and retains handle invalidation guarantees.

Reveal accepts the optional flag for older-snapshot compatibility. Presenting a
suspension change invalidates stale cue sequence IDs, while updates retain fractional
progress, rich timing and speech interval state. Finish remains a silent presentation
operation; dialogue navigation stays blocked. Locale/context refresh keeps its
existing explicit silent-completion behavior.

There is no wall clock, coroutine, thread or audio dependency in this path. Hosts
must pause their own audio/animation/external work as needed and report current-frame
dialogue time on resume.

## Qualification coverage

- Parser tests cover finite/range bounds, missing targets, mixed labels, duplicate
  waits, zero/max durations and schema-9 emission.
- Imported typed source tests verify suspension methods, boolean status, nullable
  wait records, field completion/hover and invalid input diagnostics.
- The authored wait example builds through the compiler and executes emitted Lua.
- Lifecycle tests exercise partial time, discarded excess time, zero-wait loops,
  shown/revealed event anchors, partial rich reveal, stale speech cues, suspended
  choices, queued async outcomes, synchronous-handler suspension, restart invalidation,
  invalid runtime documents and no-retry transition errors.
- TextMate recognizes the new directive through the existing grammar check.

Standalone tests use Lua 5.4.8. Native headless LÖVE 11.5 / LuaJIT 2.1.1700008891
runs the emitted example and synchronous-action, async-action and wait/suspension
oracles with JIT explicitly on and off. Both native modes passed. No installed
editor, rendered text layout or audio pause/resume playback is claimed here.

## Compiler and host provenance

Final native qualification used Tua 0.11.0, debug `aarch64-apple-darwin`, revision
`d06a376c53bdd9a6fbadbef238e559c22554a231` plus this slice's working changes.
Executable SHA-256:
`b6d50b21a2c0ed815402afeb8c487cd8160f6770fcb6ee099caf98c666f835f5`.

The example passed `tua fmt`, `tua fmt --check`, `tua check` and `tua build`
without warnings. Modified Lua runtimes, the typed contract and the maintained
Lua oracle passed the explicit shared-formatter gate separately. Final emitted
Lua passed under Lua 5.4.8 and native LÖVE with JIT on/off. No standalone Lua 5.1
or LuaJIT executable was available; LÖVE supplied the LuaJIT compatibility check.


## Repository verification

`make verify` passed, including workspace tests, Clippy, 509 LSP tests, model
reference/negative controls, 292 extension tests, grammar and metadata checks.
Existing ignored tests remain ignored.

Final review then added deferred choice-fallback reconciliation during suspension,
including rollback on a throwing resume resolver. After that runtime-only adjustment,
the complete core suite (2,152 unit tests plus emitted-runtime integrations), Clippy,
example format/check/build, Lua execution, native LÖVE JIT-on/off qualification,
docs build, model-contract drift and whitespace checks passed again. The full
model/extension gate was not repeated for this final runtime interaction.
