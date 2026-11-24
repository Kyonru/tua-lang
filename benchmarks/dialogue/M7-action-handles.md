# M7 asynchronous action-handle foundation

Implemented on `feature/tdialogue-rich-text`; not merged or released. This is one
M7 slice, not completion of voice, waits, suspension or speech playback.

## Contract and compatibility

`Dialogue.actionHandle(options?)` returns a runtime-branded, typed handle.
Handlers may return a boolean or a fresh handle; action declarations, authored
scripts and serialized schema versions are unchanged. Existing synchronous
handlers preserve immediate branch selection.

A handle has one settlement: resolved with a boolean, rejected with a bounded
message, or cancelled. It can be claimed by exactly one action visit. Each player
holds at most one handle, and completion does not retain or invoke its player.
Even settlement during the handler is consumed on a later explicit `runAction()`.
Updates and snapshots never dispatch or consume actions. No recursive async chain,
host clock, coroutine or thread is introduced.

`runAction()` returns true when a new handle is accepted or when a settled success/
failure outcome transitions successfully; it returns false while pending or on
fault. False domain outcomes select the authored failure branch. Rejections and
cancellations instead fault with action/source identity and never automatically retry.

Player restart/cancel/dispose detach the old handle after committing their lifecycle
change, then cancel it. Cleanup sees an already-cancelled handle, runs at most once,
and cannot throw through disposal. Reentrant lifecycle changes from cleanup remain
authoritative. A returned handle from an invalidated dispatch is cancelled unless
already claimed elsewhere. Settled handles do not call cancellation cleanup.

The private handle registry has weak keys and values. Handles retain state through
their method metatable. This avoids the Lua 5.1 weak-key/value cycle when cleanup
captures its own handle or player; garbage collection does not execute cleanup.
Hosts should explicitly dispose players and release external resources.

Snapshots expose action status and existing source/visit identity, not opaque handles.
Handle snapshots are owned copies. Reject messages and cleanup error messages are
bounded to 4096 bytes. A missing cleanup callback means lifecycle invalidation can
ignore late results but cannot stop external work. Timeouts are explicit host
rejection; cancellation is not rollback or an exactly-once guarantee.

## Coverage

- Imported constructors and handler results retain types; invalid resolve/reject
  inputs, result assignments and string-returning handlers fail analysis.
- Shared completion, hover and signature-help queries cover imported handles.
- Actual emitted sources exercise existing synchronous actions and asynchronous
  success/failure, immediate settlement, duplicate completion, explicit rejection,
  host timeout, cancellation, throwing cleanup and malformed/forged handles.
- Runtime oracles cover restart/cancel/dispose, stale returned handles, reentrant
  cleanup, transition failures, shared/reused handles, independent interleaved
  players, owned snapshots and GC cycles.
- The runnable `examples/dialogue-actions` demonstrates both synchronous and
  asynchronous dispatch alongside context/typed-event composition.

## Remaining M7 work

Authored duration waits, explicit suspension, non-cancellable skip policy, voice/cue
references and locale policy, character speech profiles and the optional native
audio adapter remain pending. No installed-editor interaction, graphical rendering
or audible voice playback is claimed by this headless slice.

## Native and focused validation

- Focused core action tests passed, including imported types/editor queries and
  the emitted synchronous/asynchronous runtime oracles.
- `tua fmt`/`tua fmt --check` passed for the example and explicit runtime,
  contract and Lua fixture paths. The example checks/builds without warnings.
- Lua 5.4.8 executed the emitted example successfully.
- LÖVE 11.5 / LuaJIT 2.1.1700008891 executed the emitted example and both maintained
  lifecycle oracles with JIT explicitly enabled and disabled. Both runs passed,
  including the weak-registry collection tests. No standalone Lua 5.1 or LuaJIT
  executable was available; native LÖVE supplied the LuaJIT coverage.

Compiler provenance: Tua 0.11.0, debug `aarch64-apple-darwin`, revision
`b1e22cc99aa5d9574b06ad1032fb71678e035d62` plus this slice's working changes.
Final native qualification used emitted output from executable SHA-256
`5af517ed28e67372d16d45827b89a7fcee3fe22d9150e775b1d9c0e0f06c5ee6`.

## Repository gate

`make verify` passed: workspace tests (2,150 core unit tests and 509 LSP tests),
Clippy with warnings denied, model reference/negative controls, feature mappings,
extension checks (292 tests), grammar, metadata and whitespace. Existing ignored
tests remain ignored. `make docs-build llms-drift-check diff-check` passed after
the final documentation updates. Installed-editor interaction and audio/visual
playback were not tested.
