# M4 progress: typed dialogue notifications

Baseline: `d87aa2dc`. This change completes structured notification payloads;
registered actions, outcomes, handler failures and persistent action resume policy
remain pending in M4. No automatic gameplay action, signal bus or save slot is added.

## Contract and ownership

- A `.tua` module exports `Dialogue.events({name = Save.shape(...)})`. The existing
  Save schema validator/copier and compiler payload facts are reused. Signal
  declarations describe handler argument types but do not validate/copy runtime
  data; a signal bus can remain a host-side adapter.
- `@events "module"` imports the registry. `@emit shown|revealed seconds name
  [id:stable.id] {"field":"value"}` schedules a declared notification. Explicit
  document/event IDs are required. `@event` retains its dynamic string payload.
- Authored payloads are JSON records with nested objects/arrays/scalars. Null,
  duplicate keys, nonfinite numbers, calls and expressions are rejected. Bounds:
  8 KiB JSON, 16 nested levels, 256 values, 64 total notifications per line.
  Registry construction allows 1–128 names. Programmatic data still uses the
  captured Save.shape validators and their depth/entry limits.
- Event names, required/extra fields and payload types validate against imported
  and re-exported definitions. Live schema edits invalidate diagnostics. Shared
  core queries provide event-name and top-level argument-key completion, hover,
  and navigation to the contract; nested argument-key completion remains future work.
- Registry-bound assets emit schema 5. Earlier schemas still work; the previous
  unsupported-schema fixture now uses 6. Runtime constructors reject forged or
  mismatched registry identities. Plain Dialogue.new retains legacy result typing
  and rejects registry-bound documents.
- `events.new(document)` exposes structured notification types. The `kind` and
  `name` tags both identify the declared event; legacy records have no kind.
  `events.read.name(notification)` returns a validated owned payload or nil for
  another kind/name. Corrupted matching payloads are rejected. Readers preserve
  nullable payload types through imports and re-exports.
- Context definitions opt in with `Dialogue.context({facts,locals,events=registry})`.
  Both imports must resolve to that same registry; context.new retains the existing
  fact/local APIs and adds the corresponding typed notification result contract.
- Payloads are validated/copied at player construction. Returned deliveries,
  snapshots and reader results own nested data. Input/default/copy-field mutation,
  another player and context/locale refresh cannot change captured payloads.
  Runtime payload failures include document/event identity in their error message.
- Timing, order and cancellation still use the existing player update path. No
  handler runs inside it. Hosts that restart/cancel while handling a batch must
  reject its remaining stale generation/visit tokens.

## Verification and provenance

Maintained sources:

- `examples/dialogue/events`: two contracts, repeated names, both clocks, a legacy
  notification, registry re-export and context composition.
- `crates/tua-core/tests/dialogue_events.rs`: compiler emission and Lua execution,
  malformed JSON, duplicates, bounds and required declarations/identity.
- `crates/tua-core/tests/fixtures/dialogue_typed_events.lua`: false/optional data,
  ownership across players/restarts/snapshots/readers, cancellation/disposal,
  invalid records, metatables, registry mismatch, immutable shape capture and
  forged descriptor rejection.
- Shared-analysis regression: imported payload diagnostics and editor facts,
  rejected field assignments and changing/restoring an imported schema.

Focused dialogue tests pass, as do typed example check/build/run, shared Tua/Lua
formatting and strict docs. Standalone runtime: Lua 5.4.8. The emitted example and
lifecycle oracle also pass in a headless LÖVE 11.5 host with Lua 5.1 /
LuaJIT 2.1.1700008891, confirming actual JIT status false and true in separate runs.
The native fixture's emitted dialogue runtime matches the maintained runtime bytes.
Standalone Lua 5.1/LuaJIT were unavailable. No installed-editor UI, rendered-game
or performance claim is made.

Native fixture compiler: Tua 0.11.0 development/debug aarch64-apple-darwin, baseline
plus this change, SHA-256:
`08b9e68ad3feff6905fb7ffba0303b1b7166f4d92e1584094444a7ab19b24b77`.

The broad `make verify` run passed all workspace tests: 2,126 core tests (one
existing ignored), 15 emitted dialogue integration tests, 508 LSP tests and the
CLI suites. It stopped at four Clippy findings in newly authored Rust test code
(unnecessary path allocations and borrows). After the test-only cleanup, the
focused typed-event tests passed again. The remaining gates were then run
explicitly and passed: Clippy, version/release/config/model checks, reference and
mutant evaluations, feature examples, all 292 extension tests plus grammar/theme/
activation checks, 33 JSON files and whitespace. This is completion across the
broad run and focused follow-up commands, not a claim of one uninterrupted
`make verify` pass. Production code did not change during that cleanup.

## Confirmed typing follow-up

The typed reader is useful for validation/copying and avoids relying on general
sibling-field narrowing, which is not currently supported. This independent minimal
reproduction fails under the compiler above:

```tua
type Notification = { kind: "reward", payload: { coins: number } }
  | { kind: "portrait", payload: { expression: string } }
local function inspect(event: Notification)
  if event.kind == "reward" then
    print(event.payload.coins)
  end
end
return inspect
```

Actual: TL1011 on `coins`, because payload remains the union of both records.
A future shared-core narrowing improvement should retain the matching variant after
a tag check and verify false branches, optional tags, imports, editor queries and
mutation invalidation. Do not weaken this to any or add a dialogue-only narrowing rule.
The minimal source is intentionally a negative reproduction, not a passing example.

## Reproduce

```sh
cargo build -p tua-cli
RUST_MIN_STACK=16777216 cargo test -p tua-core dialogue
make verify
make docs-build
(cd examples/dialogue/events && ../../target/debug/tua fmt --check && ../../target/debug/tua check && ../../target/debug/tua build)
(cd examples/dialogue/events/build && lua main.lua)
```

For native checks, copy the emitted example to an isolated LÖVE directory, rename
its main to `run_example.lua`, and load it followed by the maintained lifecycle
oracle from a windowless host. Confirm jit.status() for each mode. Keep the compiler
binary stable during the build-cache suite.
