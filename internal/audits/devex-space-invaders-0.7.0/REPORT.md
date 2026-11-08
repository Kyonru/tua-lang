# Space Invaders style game audit: Tua 0.7.0

Date: 2026-11-08

Repository revision at audit start: `071ada115e1cf053925073e4952ca0d5e26ef502`
Scope: exercise the protocol and polymorphism work in a small multi-file LOVE game, record issues, and fix confirmed problems found during the exercise.

## Summary

The current protocol implementation supports a useful game-code path: an
exported, method-only protocol can be imported as a type, implemented by a
class in another module, used as an array element type, and dispatched through
colon calls. The checker accepts a valid implementation, rejects missing and
incompatible implementations, and emitted Lua contains no protocol declarations
or type-only imports. A system Lua runtime smoke test exercised the generated
entry point and scored hits through the protocol-typed enemy collection.

The exercise found one confirmed Tua defect: the unused-type-import lint did
not count a name used by a class `implements` clause. This produced false
`TL4054` warnings in both class modules. The linter and regression test are now
fixed.

No other compiler or protocol failure was found in this focused exercise.
Generics remain outside what the game could use: generic aliases, generic
functions, generic classes, and protocol bounds are still future milestones in
the implementation plan. Protocol IDE behavior (completion, hover, signature
help, implementation navigation, references, and rename) was not exercised
through LSP or VS Code here. LOVE and LuaJIT were unavailable, so the runtime
check used the installed PUC Lua 5.4.8 interpreter and a small LOVE API shim;
it is not visual or target-runtime qualification.

## Coverage matrix

| Area | Result | Evidence / limit |
| --- | --- | --- |
| Multi-file project and module-root imports | Exercised | `tua check` sees five `.tua` source files; fixture uses source-root module names such as `entities.contracts`. |
| Exported protocol declaration and type-only import | Exercised | `InvaderBehavior` is exported from `src/entities/contracts.tua` and imported in `game.tua` and `invader.tua`. |
| Class `implements` and method compatibility | Exercised | Game build accepts the conforming Invader; core tests cover missing methods, incompatible parameter types, and inherited implementations. |
| Protocol-typed heterogeneous collection | Exercised | `InvaderBehavior[]` receives `Invader.new(...)` instances and dispatches `update`, `draw`, `hit`, `isAlive`, and `wouldHitEdge`. |
| Concrete member visibility through a protocol | Inspected / intentional limit | The initial experiment's concrete cast from `InvaderBehavior` to `Invader` was rejected. Protocol values expose declared methods only; the game was reshaped to express behavior in the protocol. No implicit runtime narrowing is promised. |
| Type-only runtime erasure | Exercised | Built Lua has no `protocol`, `Updatable`, or `InvaderBehavior` references; generated modules do not require `entities.contracts`. |
| Emission and runtime call path | Exercised with a shim | `tua build` emits five files. `lua runtime-smoke.lua` loads generated `main.lua`, invokes LOVE callbacks, fires shots, records score, and draws. |
| LOVE rendering, input, and actual game loop | Blocked by environment | `love` and `luajit` executables are not installed. Shim verifies call integration, not windowing, graphics, timing, or LOVE behavior. |
| LSP/editor surfaces for protocol members | Not exercised | This report makes no claim about completion, hover, signatures, definition, implementation navigation, references, rename, or extension presentation. |
| Generic types | Not applicable to this implemented slice | The 0.7.0 plan places generic aliases/functions/classes after protocol completion; no generic syntax was needed or verified by this game. |

## Confirmed finding

### SIA-001 — `implements` did not keep imported protocol types live

- **Category:** lint correctness
- **Severity:** Low
- **Confidence / status:** Confirmed and fixed; regression test passes
- **User impact:** A class could correctly import a protocol type and use it in
  `implements`, yet receive `TL4054` saying the import was unused. This adds
  noise to routine class/module work and makes a correct protocol import look
  removable.
- **Minimal reproduction:**

  ```tua
  import type { Updatable } from "entities.contracts"

  final class Player implements Updatable {
    update(dt: number): nil {}
  }
  ```

- **Observed before fix:** In the game fixture, both `player.tua` and
  `invader.tua` received `TL4054` at their `Updatable` imports although their
  class declarations implemented that protocol. The diagnostics disappeared
  after the change.
- **Root cause:** `unused_type_diagnostics` collected names used by
  annotations and aliases but did not collect
  `ClassDeclaration.implemented_protocols`.
- **Fix:** Add class conformance names to the linter's live type-name set.
- **Regression coverage:**
  `linter::tests::imported_protocol_used_by_implements_is_not_reported_unused`.
- **Recommended follow-up:** Keep all type-level declaration references in the
  same liveness walk as protocol/type syntax grows, rather than adding special
  handling to individual lint call sites.

## Protocol behavior confirmed by the game

The fixture's `InvaderBehavior` protocol is exported from one file and imported
by both its implementation and its consumer. `Invader` implements the
contract, `game.tua` stores instances in `InvaderBehavior[]`, and methods are
invoked through the protocol-typed receiver. The emitted code retains normal
class construction/metatable dispatch while erasing the protocol and type-only
imports.

The game was deliberately written so behavior needed by the consumer is part
of the contract. Direct access to concrete fields such as `x` and `alive` is
not available through the protocol view. This matches the plan's explicit
method-only, no-implicit-downcast contract; it is a design boundary rather
than a defect found by this audit.

The cross-module checker is also covered negatively: the type database test
requires a missing `update` method and a `string` parameter implementation to
be rejected, while accepting a valid imported implementation.

## Reproduction and verification

Run from the repository root:

```sh
cd internal/audits/devex-space-invaders-0.7.0
cargo run -p tua-cli -- check
cargo run -p tua-cli -- build
lua runtime-smoke.lua
```

Expected results: check reports `checked 5 file(s)`; build reports
`built 5 file(s)`; runtime prints `runtime smoke passed: score=20 wave=1`.
The smoke test uses `build/main.lua`, which installs the entry point callbacks,
and supplies only the LOVE keyboard/graphics functions used by this fixture.

Core test commands and outcomes:

```sh
cargo test -p tua-core protocol
# 8 passed

CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-core
# 1,747 passed; 1 ignored

cargo fmt --all -- --check
# passed
```

An initial default parallel core test run overflowed the test thread stack in
the existing `anonymous_nested_default_expressions_preserve_outer_helper_shadows`
case. Rerunning with the repository's larger stack and serial test settings
passed. This is a test-run configuration limitation, not evidence of a runtime
stack defect in protocol handling.

## Exercise adjustments (not Tua findings)

The initial game draft attempted to downcast protocol-typed enemies to their
concrete class to read fields. That is outside the specified protocol contract,
so the game now exposes the required behavior as protocol methods. The draft
also removed array items while iterating forward with `ipairs` and left shots
on screen after leaving the play area; the fixture was corrected to iterate
shots backwards and delete off-screen shots. These were game-source issues,
not compiler defects.

## Remaining work and limits

This is a focused behavioral audit, not qualification of all language and
editor features. Before calling the protocol milestone complete, follow the
remaining P3–P5 work in `internal/generics-polymorphism-0.7.0.md`: fresh
cross-file edit/invalidation coverage; LSP/editor parity for completion,
hover, signatures, navigation and diagnostics; user-facing protocol
documentation/examples; and actual LOVE/LuaJIT runtime qualification. Generic
alias/function/class milestones G0–G3 remain unimplemented and require their
own design and regression work.

At audit start, the worktree already contained in-progress changes for exported
protocols in `classes.rs`, `parser/tests.rs`, and `type_database/`. Those changes
were retained and tested; this report does not characterize them as changes
originating from the Space Invaders exercise.
