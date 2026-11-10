# BehaviorTree implementation report

Date: 2026-11-10. Version: 0.7.0 development build.

## Delivered

`BehaviorTree.new()` is an implicit Tua built-in. Referencing it inserts a
`require("tuabehavior")` in generated Lua and copies `tuabehavior.lua` only into
builds that need it. Lua code can import the same module explicitly. Runtime
logic is independent of LOVE and uses Lua 5.1-compatible syntax.

The module provides shareable definitions and independent actor runners,
memory and reactive sequence/selector nodes, conditions, actions with
enter/exit callbacks, waits, invert, bounded repeat, parallel all/any,
reset/cancel, and configurable per-update visit/depth limits. The public API
and semantics are in [Behavior Trees](../docs/behavior-trees.md).

This is an independent Tua implementation informed by the functional model of
[Beehive](https://github.com/drhayes/beehive.lua); no Beehive source was copied.
The built-in name follows Tua's existing `Camera` and `Rot` import mechanism.

## Verification

- `RUST_MIN_STACK=16777216 cargo test -p tua-cli --test behavior_tree` passes.
  The fixture builds Tua, runs the emitted Lua, checks independent actors,
  reactive interruption, parallel outcomes, repeat, reset/cancel, invalid
  statuses, duplicate-node rejection, visit limits, and removal of the
  bundled runtime when references disappear.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core module_types_are_structural_and_carry_their_contracts` passes; the built-in has a structural type.
- `make docs-build`, `cargo fmt --all -- --check`, and `git diff --check` pass.
- `RUST_MIN_STACK=16777216 cargo test --workspace -q` passes: 1,915 core,
  446 LSP, 139 CLI integration, and the other workspace tests passed. The
  larger test-thread stack is required by an existing deep linter test.
- `make extension-package` passes with 180 extension tests. The generated
  `editors/vscode/tua-0.7.0.vsix` passes `unzip -t` and has SHA-256
  `82f02a1e0baa31d5660160b104f549e4bf4585b5d352652b6cfaf2139b724298`.

## Remaining design work

The current built-in callback contract checks returned statuses but accepts
dynamic actor context. Generic context propagation through combinators and
cross-file callback inference require a type-system prototype. Quorum
parallel, optional tracing and inspector integration are also not shipped.
The design and acceptance criteria remain in
[Behavior Trees for Tua and LOVE](behavior-trees-love-design.md). A graphical
LOVE session and installed VS Code session have not been exercised for this
new module; its headless generated Lua path has been exercised.
