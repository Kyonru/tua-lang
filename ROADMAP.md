# Tua Roadmap

This repository-level file tracks pending product and engineering priorities.
It is an internal planning aid, not documentation of current behavior or a
release commitment.

Current user-facing behavior belongs in the focused language, tooling, and
workflow pages under `docs/`. Completed work belongs in
`CHANGELOG.md` or Git history.

## Now

### Qualify The Next Coordinated Release

- Prepare the next CLI, server and extension release together, with current
  changelog, migration notes and security guidance for applicable changes.
- Collect independent real-project evidence for deterministic inline
  completion before enabling it by default; keep exact project retrieval
  parked and the sparse statistical fallback explicitly opt-in until then.
- Fix correctness or responsiveness regressions before expanding language
  syntax.

### Deepen Everyday LOVE Workflows

- Harden asset path navigation and diagnostics across realistic project roots.
- Refine sprite-sheet preview and SignalBus graph workflows from user feedback.
- Benchmark the object-pool runtime on representative LOVE populations.
- Maintain one generated LOVE source for signatures, enums, callbacks,
  inheritance, and documentation.

### Qualify Real-Project Responsiveness And Editor Behavior

- Recheck the confirmed BYTEPATH `Director.self.timer:after` completion and
  signature failure from the November port audit against the current server.
  Preserve the full project context and assert both editor responses; if it
  still fails, minimize it and add a regression before changing shared receiver
  inference. Do not close this item from the simpler timer-adapter tests alone.
- Keep full SNKRX/BYTEPATH check and build timings as regression workloads;
  bound receiver inference and source rescanning, then profile remaining cost.
- Reduce cold project completion and post-edit dependency-surface analysis on
  the input coordinator. The November follow-up replay measured about 277 ms cold
  (previously 433 ms) and 52 ms median post-edit module completion (previously 94 ms).
  Current representative replay remains around 288 ms cold and 54 ms post-edit,
  with incomplete-edit completion near 24 ms after reducing repeated alias-map
  copies. Preserve fresh imported types and diagnostics while reusing this work.
- Preserve the recovered incomplete-edit and immediate-format performance:
  63 → 24 ms and 44 → 25 ms respectively in the follow-up replay. Repeat the real
  project replay and representative contention gate before accepting another
  optimization.
- Measure actual VS Code key-to-menu latency, pull-diagnostic client behavior,
  larger-project imports, and LOVE hot reload. Protocol replay covers dialogue,
  main, combat, import paths and format-apply-save, but not UI/rendering latency.
- Extend installed-editor qualification to prolonged real-project editing,
  Test Explorer/coverage interactions, hot reload and full-game visual parity.
  Candidate installation, real-server language features and five graphical
  LOVE/editor views passed.
  These checks do not establish long-session usability or full-port parity.
- Turn any still-unspecified reference/implementation request into an exact
  symbol, editor command, expected destination, and reproducible test before
  treating it as a confirmed defect.

Owner: LSP/project analysis and extension/runtime integration. Acceptance:
repeatable traces, before/after measurements for fixes, and recorded live-editor
results.

### Unit testing follow-ups

- Add richer Busted API declarations.
- Add an explicit LOVE runtime test mode; the Busted backend currently tests
  headless Lua logic and requires doubles for LOVE-dependent behavior.

## Next

### Improve Semantic Precision Without Losing Lua Flexibility

- Broaden control-flow and metatable inference only where identities are
  compiler-proven.
- Expand LuaDoc/LuaCATS interoperability for common existing Lua libraries.
- Improve safe project-wide refactors and module changes.
- Preserve `any` and ordinary Lua as explicit dynamic escape paths.
- Test other Lua string consumers with runtime-equivalent escaped module and
  signal names before classifying raw-token path handling as a defect. Asset
  and Sprite metadata already share the runtime string decoder; reuse it for
  another consumer only when a focused reproduction confirms the mismatch.

### Complete Typed Lua Porting Workflows

- Evaluate literal `false` support for LOVE configuration without accepting
  arbitrary booleans; use `nil` to disable window creation meanwhile.

Owner: shared semantics/checker and documentation. Acceptance: focused core/LSP
regressions and replay of the five upstream porting trials,
including ordinary functions, class bodies, unsaved edits, custom library
mutations, and runtime equivalence. Preserve full-game regression harnesses;
installed-editor, long-session, external-service and visual parity validation
remain distinct follow-up work.

### Improve Comment And Annotation Authoring

- **Generate/update documentation from parsed signatures:** support native
  class methods, multiline functions, nested table/function types, defaults,
  and varargs. Preserve prose and native types; do not invent descriptions.
- Displaying the exact default expression in hover/signature help remains a
  separate usability gap from marking a parameter optional. Define its display
  contract before changing shared signature metadata.
- **Keep comments aligned:** offer targeted fixes for stale/missing parameter
  tags and update matching documentation references during parameter rename.
  Reuse existing type-disagreement diagnostics.
- **Navigate annotations:** share native type completion/navigation for type
  names in comments. Extend tag hover to incomplete/malformed annotations;
  complete tags already explain semantics and link to the offline guide.
- **Verify editing states:** cover Lua/native functions, class methods, imports,
  partial tags, and inserted generated snippets in one conformance matrix.
  Check the resulting program and explicit source-coordinate mappings.
- For dot/colon completion, keep filtering keys that cannot be written as bare
  member names. Evaluate an explicit bracket-key completion action (including
  valid replacement ranges and suffix preservation) as a follow-up capability;
  do not emit invalid dot access to expose those keys.

Owner: core annotation metadata/semantics and LSP; the extension presents those
results rather than maintaining a second parser.

### Make Releases Easier To Trust

- Add code signing when sustainable signing identities and key management are
  available.
- Test upgrade and rollback behavior between adjacent extension/server
  versions.
- Publish compatibility and migration notes for every minor release.
- Add more architectures only when CI can build and test them natively.
- Qualify test-process cancellation and output/report bounds on native Windows,
  Linux, and macOS, including descendants that retain pipes and workspace
  changes during a Test Explorer run. Current Unix child tests do not establish
  cross-platform behavior.

## Later

### Revisit Storage And Extensibility With Evidence

- Compact generated LOVE data only if measured compile time, binary size, or
  maintenance cost justifies replacing generated Rust source.
- Evaluate protocol-like behavior types against the concrete shared update
  scheduler for unrelated gameplay actors and UI objects. Specify receiver
  binding, method compatibility, diagnostics, and navigation before replacing
  the explicit adapters in `examples/features/src/polymorphism.tua`.
- Reconsider generic classes when identity-preserving APIs cannot be modeled
  clearly with concrete types. Abstract classes/methods and multiple inheritance
  remain deferred design questions, not promised features; require use cases
  beyond those already served by single inheritance and typed adapters.
- Explore typed physics/contact ownership and checked optional configuration
  schemas only against larger ports; investigate numeric units/nominal numeric
  aliases if repeated angle, force, or dimension mistakes justify them. These
  are evidence-gathering proposals from the porting audit, not commitments.
- Keep executable third-party plugins out until ownership, ordering, security,
  and performance contracts can be made stable.

## Planning Rules

- Keep this file limited to pending outcomes, not implemented feature
  inventories.
- Do not use roadmap placement as evidence that a feature is available.
- Move implemented behavior into the relevant docs and remove the completed
  roadmap item.
- Add priorities only when ownership, user value, and a verification path are
  clear.
