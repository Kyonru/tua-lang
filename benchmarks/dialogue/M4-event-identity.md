# M4 progress: stable event identity

This is the identity foundation for M4, not completion of typed events or host
actions. Baseline dialogue implementation: `cd299d53`. Concurrent documentation
work landed as `d5f61db4` and is preserved separately.

## Contract

- `@event shown 0 pulse [id:arrival.pulse] "soft"` declares an optional stable ID.
  A logical `@document` is required. IDs must be unique across the document, even
  across nodes; they are independent of localization text IDs.
- Identified assets emit schema 4. Earlier schemas still work; assets without event
  IDs retain their previous schema. Typed context and conditions also work in 4.
- Fired records add optional `eventId`, `documentId`, and owned `source` coordinates.
  Legacy fired records do not acquire these fields. Programmatic records use `id`
  and optional `source = {line, column}`; positions must be positive integers.
- IDs survive rewording, declaration reordering and file renames. Coordinates do
  not. Source metadata is diagnostic, never an executable path.
- `generation` and `visit` remain per-player delivery tokens. Repeated visits can
  deliver the same authored event again. This does not implement persistent action
  execution, restore/deduplication, callbacks, structured payloads or result routing.
- Runtime ingestion owns the metadata before playback. Caller changes to the
  input, snapshots or returned event source cannot modify future deliveries.
- The shared typed contract supplies nullable result fields through re-exports;
  core tests cover completion, hover and rejected field assignments. The extension
  grammar recognizes the ID tag. No extension-only semantic index was added.

## Evidence

Maintained fixtures:

- `examples/dialogue/event-identity`: typed host, two occurrences of the same name,
  both clocks, source coordinates and a new delivery after restart.
- `crates/tua-core/tests/fixtures/dialogue_event_identity.lua`: metadata ownership,
  locale refresh, cancellation, advance, disposal, repeated names/reordered
  declarations, invalid identity/source data and document-wide uniqueness.
- `crates/tua-core/tests/dialogue_conditions.rs`: runs the emitted typed context,
  conditional script and lifecycle checks with both schema 3 and identified schema 4.

The example passes check/build and executes under standalone Lua 5.4.8. Explicit
Tua/Lua formatting checks pass. The emitted identity fixture also passes in a
headless LÖVE 11.5 host, Lua 5.1 / LuaJIT 2.1.1700008891, with JIT status confirmed
false and true in separate runs. The native host used compiler-emitted documents
and the emitted runtime, verified byte-for-byte against the runtime source.
Standalone Lua 5.1/LuaJIT executables were unavailable. No installed-editor UI,
rendered-game or performance claim is made.

Compiler: Tua 0.11.0 development/debug aarch64-apple-darwin, with this change;
SHA-256 `3a9227195fa02b7a355e56c1a869d6306dd760474f3402c9637d3535a10aa206`.
The reference and revised documents were built as separate projects because the
project checker correctly rejects duplicate document/text IDs within one project.

An initial oracle incorrectly expected a locale change to leave reveal pending.
Existing behavior completes reveal on locale refresh. The corrected oracle allows
the pending revealed event and verifies that previously fired events are not replayed;
production locale/timing behavior was not changed to satisfy the test.

The first broad gate stopped in unrelated parallel RNG fixtures: one project lost
its working directory and another observed the wrong program output. Their names
used PID plus timestamp without a per-process sequence. The separate test-only
commit `b777df30` adds the same unique sequence used by the dialogue fixtures; all
four RNG tests pass after this isolation fix. That first broad run is not counted
as passing.

The second `make verify` passed the workspace tests (2,125 core tests plus emitted
integration suites, 508 LSP tests, and CLI suites), Clippy, model-contract/reference/
mutant checks and feature examples. It stopped at the extension's generated-guide
check: concurrent commit `d5f61db4` had improved `docs/annotations.md` directly.
Separate commit `7cb50fd5` preserves that wording in canonical annotation metadata
and regenerates the offline guide. All 52 focused annotation tests then pass.
`make extension-check json-check diff-check` passes afterward: 292 extension unit
tests, syntax/theme/manifest/activation checks and 33 JSON files. Strict docs and
explicit shared-formatting checks also pass. The whole gate was not rerun a third
time; its remaining checks were completed separately after the metadata-only fix.

## Reproduce

```sh
cargo build -p tua-cli
RUST_MIN_STACK=16777216 cargo test -p tua-core --test dialogue_asset
RUST_MIN_STACK=16777216 cargo test -p tua-core dialogue
make verify
make docs-build
(cd examples/dialogue/event-identity && ../../target/debug/tua fmt --check && ../../target/debug/tua check && ../../target/debug/tua build)
(cd examples/dialogue/event-identity/build && lua main.lua)
```

Hold the compiler binary stable while the build-cache verification suite runs.
For native verification, compile the original and revised documents from
`identified_events_preserve_identity_after_edits_and_execute_in_emitted_lua`
as separate projects, then load both emitted modules with the maintained Lua oracle
in a windowless LÖVE host. Assert actual JIT status for each run.

Next: imported typed event/action contracts, structured payload validation and
capture, host action outcomes and failure/cancellation policies. The M4 checklist
remains open until those surfaces and their acceptance tests are implemented.
