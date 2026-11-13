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
- Qualify large populated module, signal, state-machine, and ECS previews against
  compiler output and source targets. Then add cross-view relationship links
  only where stable semantic IDs and ranges identify both ends; keep source
  navigation separate.
- Define a compiler-owned input binding map before adding a binding preview.
  Evaluate a static store schema/watcher view separately from the existing
  live Runtime Inspector history; avoid presenting static links as runtime
  events. Qualify native asset previews and long inspector sessions in a real
  game project.
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
- A November 11 installed-editor follow-up measured visible suggestions in a
  synthetic 113-file project: imported-table
  completion was about 161 ms warm median; a one-hop imported class factory was
  about 108 ms. Pointer hover on that class took 320–325 ms including VS Code's
  hover delay, and format-on-save of 1,001 lines took 16–33 ms. Trace trigger,
  client send, server response, and widget paint before assigning the visible
  delay to a component. Repeat on a real game and previous version; also test
  pull-diagnostic client behavior, larger imports, and LOVE hot reload.
- Investigate the November 10 release-build LSP stress result: a synthetic
  1,000-module workspace did not finish profiling after 270 seconds; its active
  server was sampled at about 3.75 GiB RSS and 299% CPU. The same profile had
  sub-millisecond warm completion p95 on the 100-module fixture. The 1,000-file
  run was stopped before a response or root cause was established; do not
  generalize the stress result to normal projects. Reproduce with
  `TUA_REPRESENTATIVE_COLD_REPEAT=5 TUA_REPRESENTATIVE_WARM_REPEAT=20
  TUA_REPRESENTATIVE_CONTENTION_REPEAT=20 make profile-representative`, then
  profile workspace warming and retained per-file/snapshot data before
  proposing a code change. A bounded November 11 replay with one sample per
  request completed: cold completion was 2.96 seconds, cold hover 2.81 seconds,
  cold definition 6.91 seconds, and warm completion 9.32 ms; sampled peak RSS
  was about 1.37 GiB. A three-sample comparison of direct require-binding
  lookup reduced average cold definition from 7.23 to 3.82 seconds. Average
  cold completion (2.79 seconds) and hover (2.75–2.81 seconds) remained
  essentially unchanged, and sampled peak RSS remained about 1.4 GiB. The
  trace points to further project semantic-environment work on the entry file;
  isolate its expensive substeps before changing caches. The one-round
  one-round contention probe returned only an invalid-document completion in
  both runs, so it did not qualify valid-document delivery under edits. A
  November 11 release-candidate
  replay with matched settings reproduced about 1.40 GiB peak RSS and 2.82-second
  cold completion. Three contended edits in the same 1,000-module fixture raised
  sampled peak RSS to 2.52 GiB and post-edit completion p95 to 1.85 seconds;
  two valid-document completions arrived without stale results. Investigate
  retained memory before qualifying very large workspaces. The local 17-file
  LOVE project had 441–457 ms cold cross-file completion and 83–90 ms on later
  edits.
  The 0.9.0 priority audit reproduced 2.92-second cold completion and 1.44 GiB
  sampled RSS on the released 0.8.0 server with one bounded 1,000-module run.
  Its span trace pointed to repeated entry-file export and snapshot work.
  Reusing a semantic model within each module export pass reduced matched
  0.9.0 cold completion from 3.02 to 2.66 seconds and cold hover from 3.10 to
  2.64 seconds; warm completion stayed near 9 ms and sampled peak RSS rose
  about 3%. The remaining multi-second cold cost and memory need a real-game
  replay and further shared-analysis profiling. A follow-up skip of unused
  import aliases in export indexing reduced matched cold completion from 2.75
  to 2.28 seconds, with sampled peak memory steady near 2.69 GiB. The result
  remains a synthetic protocol replay, so visible editor timing and real-game
  memory are still open.
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
  not qualified.
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

- Add a Test Explorer Debug profile only after `tua test` can prepare a
  persistent, source-mapped test build for a debugger to launch. Preserve the
  selected Busted case filter, use the configured Lua interpreter, keep the
  output alive until the debug session ends, and clean it on cancellation.
  Verify breakpoints in both a `.tua` spec and its subject in an installed host;
  a profile that merely launches Busted without mapped breakpoints is not done.
- Design an explicit LOVE runtime test mode with clear discovery, launch,
  cancellation, exit-status, and result-reporting behavior. The Busted backend
  currently tests headless Lua logic and needs doubles for LOVE-dependent code.
- Qualify Test Explorer continuous runs and line coverage in the installed
  editor with real Busted and LuaCov. A VS Code 1.142 extension-host replay
  confirmed selected-spec execution and cancellation; continuous-run
  interaction and coverage UI remain open. LuaCov installation could not
  reach the configured LuaRocks mirrors in the audit environment.

## Next

### Close Tua-Specific LSP Workflow Gaps

The Kotlin LSP comparison found no missing standard request from its published
feature list: Tua already supports completion, diagnostics/actions, semantic
tokens, import organization, rename, formatting, documentation/hover, call
hierarchy, and folding. Focus on behavior depth instead of duplicate endpoints.
The comparison also reviewed source-license boundaries.

- Audit navigation for configured Lua dependencies and generated declarations;
  link to indexed source or stable docs where available, and stay conservative
  for dynamic modules.
- Measure cold/open and large-workspace request latency before adding progress
  UI or persistent indexes. Reuse current health/cache telemetry and keep
  interactive requests cancellable.
- Select future refactorings from repeated Tua workflows and add one safe,
  semantically validated action at a time.
- Maintain a small non-VS-Code LSP-client qualification matrix for protocol
  compatibility; keep testing/debug integration in their appropriate client
  APIs.

Owner: shared core queries with thin LSP/extension adapters. Acceptance:
focused core tests, protocol payload tests, and source-aware edit tests; no
parallel parser or semantic model in the extension.

### Improve Semantic Precision Without Losing Lua Flexibility

- Add a bounded type-alias parameter contract before promising direct exported
  aliases of protocol-bounded generic classes. The current `Jar<T: Named>` /
  `JarInstance<T> = Jar<T>` form fails at the alias declaration, while the
  proposed `JarInstance<T: Named>` spelling does not parse. Keep the checked
  unbounded-class/bounded-factory pattern documented meanwhile. Acceptance for
  new syntax: same-file and imported positive/negative checks, alias
  application and class-bound diagnostics, completion/hover/navigation, and
  unchanged emitted Lua. See the November 13 game-trial follow-up.
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
