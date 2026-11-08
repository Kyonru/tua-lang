# Tua Roadmap

This repository-level file tracks pending product and engineering priorities.
It is an internal planning aid, not documentation of current behavior or a
release commitment.

Current user-facing behavior belongs in the focused language, tooling, and
workflow pages under `docs/`. Completed work belongs in
`CHANGELOG.md` or Git history.

## Now

### 0.7.0: Protocol Contracts First, Then Generics

The primary 0.7.0 goal is stronger polymorphism and generics. The initial
method-only protocol syntax, explicit class conformance, basic checking, and
cross-module type imports are implemented. The complete protocol editor and
qualification slice and all native generics remain open; this is not a release
completion claim. The detailed contributor plan is kept locally in
`internal/generics-polymorphism-0.7.0.md`.

1. **Complete protocol semantics and module freshness.** The compiler now
   supports method-only declarations, explicit `implements`, protocol-typed
   parameters/arrays, basic missing and incompatible method diagnostics,
   inherited method satisfaction, and exported protocols through type-only
   imports. Verify inherited conformance, multiple obligations, variance edge
   cases, declaration-module contracts, re-exports, and unsaved dependency edits.
   Keep class identity nominal and erase contracts without runtime wrappers.
2. **Ship complete protocol editor support.** Completion, hover, signature help,
   inlay hints, definition, references, rename, and implementation navigation
   must agree on protocol and concrete member identity, including unsaved edits
   and incomplete declarations. Core/LSP request coverage now exercises
   completion, hover, signature help, definition, and implementation navigation
   across type-only imports; protocol documentation propagation, inlays,
   references/rename, tokens, type explanations, and installed VS Code remain
   unverified. Use the shared model rather than feature-local conformance checks.
3. **Add scoped generic aliases and functions.** Introduce declaration-owned
   type parameters and shared substitution/inference; preserve return tuples,
   callback signatures, imports, and declaration identity. Make unsupported
   forms explicit instead of silently degrading to `any`. Route LuaDoc generic
   annotations through the same semantics when their contracts are supported.
4. **Add generic classes and protocol constraints.** Reuse the same substitution
   model for construction, inherited members, `Self`, and protocol bounds.
   Decide mutable-container variance before accepting generic widening; current
   array subtyping is not evidence that mutable generic containers are safe.
5. **Qualify the release.** A buildable protocol example and an audit-only game
   fixture now exercise emitted Lua and dispatch; actual LOVE/LuaJIT behavior,
   source maps, hot reload, cross-platform CI, LSP, installed-editor, and
   responsiveness gates remain pending. Keep examples and public documentation
   aligned as capabilities land, with explicit migration notes.

Owner: core parser/semantic/checker/emitter and IDE queries; LSP and VS Code
consume shared results. Each behavior lands with its own tests and documentation.
First acceptance case: an actor and a UI panel implement the same update
contract, share an update loop, and retain their own implementation at runtime;
missing methods and unsafe signatures fail at the implementation declaration.

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

- Keep the BYTEPATH `Director.self.timer:after` full-project regression as a
  named receiver typing fixture. Its typed legacy class contract and
  saved/unsaved completion/signature regression are committed. A 10-minute
  full-project replay passed 114 edit cycles without feature failures; repeat
  the run while investigating the observed RSS growth before treating long
  sessions as qualified.
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
- The BYTEPATH memory investigation traced the large cold cost to duplicated
  parse and analysis ownership: the parse cache and analysis files separately
  retained ASTs, last-good recovery copied AST/HIR/type environments, cached
  files duplicated source strings, and cached snapshots cloned project-wide
  indexes. Sharing immutable parse/file/snapshot data cut a same-host full
  BYTEPATH `tua check` peak from 2.17 GiB to 1.08 GiB. The earlier 3.68 GiB
  editor jump was not reproduced: the final 10-minute replay stayed around
  1.82–2.01 GiB RSS. It still grew by roughly 185 MiB over ten minutes,
  however, so sustained memory growth remains unexplained and long sessions are
  not qualified. See the local-only
  `internal/audits/devex-2026-11-06/memory-investigation-2026-11-06.md` and
  `workflow/soak-evidence.json` for host, binary hashes, and detailed results.
  Keep profiling the retained growth separately from the now-reduced cold peak;
  do not infer a memory leak solely from RSS or treat a single ten-minute replay
  as long-session qualification.
- Turn any still-unspecified reference/implementation request into an exact
  symbol, editor command, expected destination, and reproducible test before
  treating it as a confirmed defect.

Owner: LSP/project analysis and extension/runtime integration. Acceptance:
repeatable traces, before/after measurements for fixes, and recorded live-editor
results.

### Unit testing follow-ups

- Design an explicit LOVE runtime test mode with clear discovery, launch,
  cancellation, exit-status, and result-reporting behavior. The Busted backend
  currently tests headless Lua logic and needs doubles for LOVE-dependent code.
- Qualify Test Explorer continuous runs and line coverage in the installed
  editor with real Busted and LuaCov. A VS Code 1.142 extension-host replay
  confirmed selected-spec execution and cancellation; continuous-run
  interaction and coverage UI remain open. LuaCov installation could not
  reach the configured LuaRocks mirrors in the audit environment.

## Next

### Improve Semantic Precision Without Losing Lua Flexibility

- Broaden control-flow and metatable inference only where identities are
  compiler-proven.
- Expand LuaDoc/LuaCATS interoperability for common existing Lua libraries.
- Improve safe project-wide refactors and module changes.
- Preserve `any` and ordinary Lua as explicit dynamic escape paths.

### Complete Typed Lua Porting Workflows

Owner: shared semantics/checker and documentation. Acceptance: focused core/LSP
regressions and replay of the five upstream porting trials,
including ordinary functions, class bodies, unsaved edits, custom library
mutations, and runtime equivalence. Preserve full-game regression harnesses;
installed-editor, long-session, external-service and visual parity validation
remain distinct follow-up work.

### Improve Comment And Annotation Authoring

- For dot/colon completion, keep filtering keys that cannot be written as bare
  member names. Evaluate an explicit bracket-key completion action (including
  valid replacement ranges and suffix preservation), exact default-expression
  display, and nullable-versus-optional overload ranking as follow-up
  capabilities; do not emit invalid dot access to expose bracket-only keys.

Owner: core annotation metadata/semantics and LSP; the extension presents those
results rather than maintaining a second parser.

### Make Releases Easier To Trust

- Add code signing when sustainable signing identities and key management are
  available.
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
- Define ownership, registration, and initialization order for dynamic service
  tables and mixins before proposing syntax or implicit lifecycle changes.
- Abstract classes/methods and multiple inheritance remain deferred design
  questions; require use cases beyond single inheritance and protocol contracts.
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
