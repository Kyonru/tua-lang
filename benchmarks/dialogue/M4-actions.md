# M4 registered host actions

Implemented on the Tua checkout based on `6c3ba3c4b54cceb2d6b041313d77072a87961c43`.
The compiler still identifies as 0.11.0 with local changes; no release was published.

## Contracts exercised

- `Dialogue.actions` captures validated Save shapes and effect classifications.
- Action-only labels use explicit document/action identity, bounded JSON payloads,
  and validated success/failure targets. Action and event IDs cannot collide.
- Imports/re-exports share payload diagnostics, completion, hover and definition
  navigation. Constructor handler arguments and results retain their types through
  optional options. A negative regression covers both mismatches.
- Only explicit `runAction()` dispatches a synchronous handler. Waiting snapshots
  contain request metadata, not an utterance. A false handler result selects the
  failure target; successful dispatch still returns true.
- Emitted fixtures cover missing handlers, errors, invalid results, recursive
  dispatch, cancel/restart/dispose during dispatch, stale errors, payload ownership,
  copied handler maps, repeated names at distinct IDs, loops, forged registries,
  destination errors and transition-budget faults.
- Context changes inside handlers affect subsequent branches. Schema 6 retains
  shown/revealed notification clocks. Older schema/event fixtures remain in the
  dialogue suite, including rejection of unsupported schema 7.

## Limits

Handlers must return synchronously. Coroutine yielding and asynchronous handles
are not a supported contract (M7). Effect classification records intent; it does
not roll back host mutations or guarantee exactly-once delivery. There is no
checkpoint/restore API for pending actions (M8). Hosts reconcile side effects
before explicitly restarting a failed action.

Dynamic Lua documents receive runtime validation. Authored scripts use shared
registry facts; asset exports retain the general document shape. Completion covers
names and top-level payload keys. Nested key completion and general tagged-union
sibling narrowing remain separate follow-ups from the typed-event milestone.

## Reproduce

```sh
cargo build -p tua-cli
RUST_MIN_STACK=16777216 cargo test -p tua-core dialogue
make verify
make docs-build
(cd examples/dialogue/actions && ../../target/debug/tua fmt --check && ../../target/debug/tua check && ../../target/debug/tua build)
(cd examples/dialogue/actions/build && lua main.lua)
```

The maintained Lua oracle is `crates/tua-core/tests/fixtures/dialogue_actions.lua`.
The integration test emits the typed example modules before executing it. Native
checks copy the compiler-emitted modules into an isolated windowless LÖVE host,
then run the example and the same oracle, explicitly calling `jit.on()`/`jit.off()`.
Standalone Lua 5.1 and standalone LuaJIT are unavailable on this host; LÖVE supplies
LuaJIT. No rendering, installed-editor UI, performance or audio claim is made.

## Verification results

Final compiler: version 0.11.0, base revision above with local changes,
`aarch64-apple-darwin`, debug profile. Executable SHA-256:
`ebd67190760aaa1c317bba94d0f715ea68b46ffe328bb89d9b52d91c044d21c4`.

- Final `cargo test -p tua-core`: 2,127 passed, one ignored, plus 17 integration
  tests passed. Focused dialogue checks include 29 core tests.
- Example formatting/check/build and execution pass with the final compiler.
- Explicit formatting gates pass for the contract, authored runtime Lua and Lua
  oracles; `cargo fmt --check` is a separate Rust gate.
- Emitted lifecycle checks pass in Lua 5.4.8 and LÖVE 11.5.0 /
  LuaJIT 2.1.1700008891, with `jit.status()` confirmed true and false in separate runs.
- Action directive grammar checks pass; the shared analyzer regression covers
  imported/re-exported names, payload keys, navigation and diagnostic refresh.

During implementation, a negative handler test exposed an optional-options union
that bypassed handler specialization. It was fixed and captured as a maintained
regression; the final compiler rejects the invalid callback with TL1003 at
`options.handlers.purchase`. The complete core suite and native checks above were
rerun after that fix. The compiler binary was not replaced during the CLI cache
suite. No installed VS Code UI check or standalone Lua 5.1 run was available.

The broad `make verify` run completed successfully: workspace/CLI tests, 508 LSP
tests, Clippy with warnings denied, version/release/schema/model checks, reference
and mutant evaluations, feature examples, 292 extension tests plus grammar/theme/
activation checks, JSON validation and whitespace. The final core rerun noted
above follows the optional-handler fix and shared editor lookup cleanup made while
the broad run was progressing. Final example/native checks use the recorded rebuilt
binary, not the earlier compiler. Strict `make docs-build` also passed after the
plan and documentation updates. No benchmark timing claims are made.
