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

- Keep full SNKRX/BYTEPATH check and build timings as regression workloads;
  bound receiver inference and source rescanning, then profile remaining cost.
- Reduce cold project completion and post-edit dependency-surface analysis on
  the input coordinator. The November follow-up replay measured about 277 ms cold
  (previously 433 ms) and 52 ms median post-edit module completion (previously 94 ms).
  The ordered-backlog replay retains roughly 288 ms cold and 54 ms post-edit,
  with incomplete-edit completion at 24 ms after removing repeated alias-map
  copies. See `internal/ordered-backlog-2026-11.md` for all rows, including hover
  costs. Preserve fresh imported types and diagnostics while reusing this work.
- Preserve the recovered incomplete-edit and immediate-format performance:
  63 → 24 ms and 44 → 25 ms respectively in the follow-up replay. Repeat the real
  project replay and representative contention gate before accepting another
  optimization. Evidence: `internal/completion-latency-followup-2026-11.md`.
- Measure actual VS Code key-to-menu latency, pull-diagnostic client behavior,
  larger-project imports, and LOVE hot reload. Protocol replay covers dialogue,
  main, combat, import paths and format-apply-save, but not UI/rendering latency.
- Test the newly packaged VSIX in an installed editor and a graphical LOVE game.
  Headless tests and synthetic trials do not establish interactive usability,
  reload behavior, or large-project performance.
- Turn any still-unspecified reference/implementation request into an exact
  symbol, editor command, expected destination, and reproducible test before
  treating it as a confirmed defect.

Owner: LSP/project analysis and extension/runtime integration. Acceptance:
repeatable traces, before/after measurements for fixes, and recorded live-editor
results. Evidence: [development trials](internal/devex-trials-2026-11.md).

### Unit testing follow-ups

- Add a dedicated test source root and production/package exclusions for specs;
  the initial `tua test` command discovers regular sources under `source`.
- Add VS Code Test Explorer discovery, individual test execution, watch-mode
  testing, richer Busted API declarations, and coverage/report integration.
- Add an explicit LOVE runtime test mode; the Busted backend currently tests
  headless Lua logic and requires doubles for LOVE-dependent behavior.

## Next

### Improve Semantic Precision Without Losing Lua Flexibility

- Broaden control-flow and metatable inference only where identities are
  compiler-proven.
- Expand LuaDoc/LuaCATS interoperability for common existing Lua libraries.
- Improve safe project-wide refactors and module changes.
- Preserve `any` and ordinary Lua as explicit dynamic escape paths.

### Complete Typed Lua Porting Workflows

- Evaluate literal `false` support for LOVE configuration without accepting
  arbitrary booleans; use `nil` to disable window creation meanwhile.

Owner: shared semantics/checker and documentation. Acceptance: focused core/LSP
regressions and replay of the [five upstream porting trials](internal/upstream-port-audit-2026-11.md),
including ordinary functions, class bodies, unsaved edits, custom library
mutations, and runtime equivalence. Preserve full-game regression harnesses;
installed-editor, long-session, external-service and visual parity validation
remain distinct follow-up work.

### Improve Comment And Annotation Authoring

- **Generate/update documentation from parsed signatures:** support native
  class methods, multiline functions, nested table/function types, defaults,
  and varargs. Preserve prose and native types; do not invent descriptions.
- **Keep comments aligned:** offer targeted fixes for stale/missing parameter
  tags and update matching documentation references during parameter rename.
  Reuse existing type-disagreement diagnostics.
- **Navigate annotations:** share native type completion/navigation for type
  names in comments. Extend tag hover to incomplete/malformed annotations;
  complete tags already explain semantics and link to the offline guide.
- **Verify editing states:** cover Lua/native functions, class methods, imports,
  partial tags, and inserted generated snippets in one conformance matrix.
  Check the resulting program and explicit source-coordinate mappings.

Owner: core annotation metadata/semantics and LSP; the extension presents those
results rather than maintaining a second parser. Acceptance and rationale:
[documentation review](internal/documentation-review-2026-11.md).

### Make Releases Easier To Trust

- Add code signing when sustainable signing identities and key management are
  available.
- Test upgrade and rollback behavior between adjacent extension/server
  versions.
- Publish compatibility and migration notes for every minor release.
- Add more architectures only when CI can build and test them natively.

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
