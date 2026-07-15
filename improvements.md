# Tua Improvement Backlog

Repository-wide audit performed on 2026-07-15. Priorities reflect user impact,
correctness risk, architectural leverage, and release readiness.

## Highest Priority

### P0: Fix the bundled LOVE project regression

`cargo test --workspace` currently rejects valid LOVE calls in the bundled
`love-full` example:

- `love.graphics.draw(Image, ...)` is rejected because `Image` is not treated
  as assignable to `Drawable`.
- Two-argument `love.graphics.newFont(path, size)` calls are rejected even
  though the generated signature catalog contains the overload.

Likely causes are missing LOVE inheritance in
[`assignability.rs`](crates/tua-core/src/types/assignability.rs) and checker/API
catalog drift between [`love_api.rs`](crates/tua-core/src/love_api.rs) and
[`love_api_signatures.rs`](crates/tua-core/src/love_api_signatures.rs).

Improvements:

- Represent LOVE type inheritance in the shared type database.
- Make argument checking consume the authoritative overload catalog.
- Generate functions, overloads, enums, and type hierarchy from one pinned
  LOVE API source.
- Add generated-catalog drift checks and the LOVE example test to CI.

### P0: Make LSP work preemptible

The server dequeues and handles each message synchronously in
[`server/mod.rs`](crates/tua-lsp/src/server/mod.rs). Queue prioritization only
affects requests that have not started. An expensive hover, inlay hint, symbol,
or diagnostic request can therefore block an incoming document change and
completion request for several seconds.

`$/cancelRequest` is not handled as an actual cancellation signal in
[`handlers.rs`](crates/tua-lsp/src/server/handlers.rs), and inlay hints are
currently exempted from part of the background-yield behavior.

Improvements:

- Separate transport/document synchronization from analysis workers.
- Give every request a cancellation token and document revision.
- Cancel or discard stale background results after a document change.
- Prioritize `didChange`, completion, signature help, hover, and definition
  over diagnostics and whole-document background features.
- Coalesce duplicate inlay, semantic token, symbol, and diagnostic requests.
- Add tests where typing arrives while a slow request is already executing.

### P0: Remove the extension suggestion race

[`suggestions.js`](editors/vscode/src/editor/suggestions.js) invokes
`editor.action.triggerSuggest` after a fixed 25 ms timer for nearly every typed
identifier. This can issue duplicate completion requests and request results
for a stale document version or cursor position.

Improvements:

- Prefer VS Code quick suggestions and the normal LSP completion provider.
- Remove the forced trigger or gate it behind an explicit compatibility
  setting.
- If a workaround remains necessary, wait for document synchronization rather
  than using a blind timer.
- Add real VS Code tests that type into a document and assert request version,
  cursor position, result freshness, and visible suggestions.

## Core Architecture

### P1: Replace per-request project scans with incremental indexes

Completion currently collects project source text in
[`analysis/mod.rs`](crates/tua-core/src/analysis/mod.rs), while ranking and
signal event completion can scan or reparse project files in
[`completion/mod.rs`](crates/tua-core/src/completion/mod.rs).

Improvements:

- Store revisioned per-file member usage, event schema, event usage, and symbol
  proximity summaries.
- Update summaries only for changed files.
- Aggregate project ranking data without retaining or rescanning source text.
- Cache completion candidates and rankings by revision and semantic context.
- Keep compiler visibility and type facts authoritative over ranking signals.

### P1: Make watch builds dependency-aware

[`watch/mod.rs`](crates/tua-cli/src/watch/mod.rs) passes only changed files to a
fresh analysis host in [`build/mod.rs`](crates/tua-cli/src/build/mod.rs). An
exported API change can therefore leave unchanged importers unchecked or
stale.

Improvements:

- Keep one long-lived project analysis host in watch mode.
- Apply file additions, edits, removals, and renames incrementally.
- Compute the reverse dependency closure for changed module surfaces.
- Recheck and emit affected dependents.
- Test changing an exported type and observing a new diagnostic in an
  unchanged importer.

### P1: Complete or narrow the plugin architecture

The registry in [`plugin/mod.rs`](crates/tua-core/src/plugin/mod.rs) advertises
declarations, diagnostics, completions, hover snippets, and structured
lowering requests. Production analysis visibly consumes plugin diagnostics,
but sprite completion and emission still use direct hard-coded paths.

Improvements:

- Route plugin declarations, completion, hover, and lowering through shared
  cached analysis queries.
- Define ownership, ordering, conflict, and diagnostic-code rules.
- Keep third-party executable plugins disabled until API stability and trust
  boundaries are established.
- Remove unsupported trait surfaces if they are not ready to be integrated.
- Keep ordinary Lua-compatible APIs, such as `spriteSheet({...})`, preferred
  over new syntax unless a plugin demonstrates clear semantic value.

### P1: Introduce structured lowering

[`emit/mod.rs`](crates/tua-core/src/emit/mod.rs) emits text and then applies a
sequence of text transformations for defaults, built-ins, imports, runtime
checks, classes, compound assignments, and sprites. This is difficult to
compose and makes source mapping fragile.

Improvements:

- Lower typed syntax and plugin requests into a small AST or lowering IR.
- Preserve source provenance through every transformed node.
- Format once after lowering.
- Add golden output and traceback/source-map tests for mixed transformations.

### P1: Split semantic analysis into stable query boundaries

[`analysis/mod.rs`](crates/tua-core/src/analysis/mod.rs) contains host state,
snapshots, caching, recovery, project indexing, completion, hover, navigation,
and diagnostic orchestration in more than 6,500 lines.

Improvements:

- Split host, snapshot, database, recovery, and feature query modules.
- Make cached semantic facts mandatory for valid parsed files.
- Define one explicit degraded path for temporarily invalid source.
- Remove repeated environment construction and recursive fallback behavior.
- Track query dependencies and invalidation at semantic surface granularity.

## Quality And Delivery

### P1: Add CI and restore strict linting

The repository has no CI workflow. The strict command
`cargo clippy --workspace --all-targets -- -D warnings` currently reports 20
errors, including overly broad completion APIs and concentrated complexity in
analysis and emission.

Improvements:

- Run `cargo fmt --all -- --check`, Clippy, workspace tests, extension checks,
  documentation builds, JSON validation, and generated-data drift checks.
- Run on Linux, macOS, and Windows where platform behavior differs.
- Keep formatting commands in verification non-mutating.
- Treat bundled example failures as release blockers.
- Add a smaller non-flaky benchmark smoke test and retain full profiles as
  trend data.

### P1: Use representative performance benchmarks

The current medium fixture is 102 lines and the large fixture is 149 lines.
The cheatsheet that exposed multi-second editor contention is 764 lines. The
LSP profiler also sends requests serially, so it cannot reproduce queue
contention.

Improvements:

- Generate deterministic 1,000-line and 5,000-line single-file fixtures.
- Add 100-file and 1,000-file project fixtures.
- Benchmark cold and warm completion, hover, definition, and inlay hints.
- Simulate bursts of background requests followed by `didChange` and
  completion.
- Record p50, p95, p99, request age, cache hit rate, cancellation rate, and
  stale-result count.
- Fail regression checks when completion freshness or a generous latency
  budget is violated.

### P1: Add real VS Code integration tests

The extension check in [`package.json`](editors/vscode/package.json) validates
JavaScript syntax, the manifest, and mocked activation. It does not exercise a
real extension host, editor document synchronization, or webviews.

Improvements:

- Use `@vscode/test-electron` for extension-host tests.
- Cover automatic suggestions after one character and continued filtering.
- Cover Cmd+click definition, hover, signature help, and inlay hints while
  typing.
- Cover rapid edits, cancellation, UTF-16 positions, untitled documents,
  multi-root workspaces, file renames, and stale responses.
- Test sprite preview opening, reload, top-most behavior, asset errors, and
  hover commands.

### P1: Split oversized test modules

`crates/tua-lsp/src/server/tests.rs` has more than 8,000 lines.

Improvements:

- Split tests by completion, hover, navigation, diagnostics, workspace,
  formatting, semantic tokens, inlay hints, and scheduling.
- Share protocol fixtures and request helpers.
- Keep regression tests next to the behavior they protect.

## Extension And LOVE Developer Experience

### P2: Move sprite semantics into the language server

[`sprite-preview.js`](editors/vscode/src/sprite-preview.js) registers a separate
hover provider and contains ad hoc parsing, asset resolution, webview state,
and rendering in one large module. This duplicates compiler behavior.

Improvements:

- Expose structured sprite metadata through a general LSP inspector request.
- Resolve sprite declarations, animation ranges, and assets from the semantic
  snapshot or plugin index.
- Keep the extension responsible only for preview presentation.
- Split preview panel state, view model, webview template, and webview runtime.
- Surface diagnostics for missing assets, invalid frames, and conflicting
  animation definitions consistently across compiler and preview.

### P2: Expand LOVE-focused semantic support

Improvements:

- Complete LOVE object inheritance and overload handling first.
- Add asset path definition and reference navigation.
- Add callback skeleton completion and callback parameter documentation.
- Improve `conf.lua` configuration completion and validation.
- Map emitted Lua runtime errors back to Tua source positions.
- Keep LOVE catalog documentation, signatures, enum values, and object methods
  generated from one authoritative version.

### P2: Complete SignalBus intelligence

Improvements:

- Maintain a semantic event index by bus identity and event name.
- Infer payload schemas from typed listeners and `declare` callbacks.
- Diagnose conflicting listener/declaration payloads.
- Provide event-name completion, signature help, hover, definition,
  references, and rename from the same index.
- Rank by same bus, same file, declaration proximity, and project usage.
- Keep untyped buses permissive and avoid introducing new language syntax.

## Language And Type System

### P2: Define inferred variable reassignment semantics

Inferred locals can currently drift toward permissive behavior unless they
have explicit annotations.

Improvements:

- Decide whether an inferred local has a stable widened type within its scope.
- Diagnose incompatible reassignment in `.tua` code while preserving explicit
  `any` as the dynamic escape hatch.
- Handle control-flow joins and nil initialization predictably.
- Document the distinction between Tua checking and ordinary Lua behavior.

### P2: Prioritize native function types before generics

LuaDoc function types, overloads, and typed varargs already provide useful
behavior, but native Tua function-type syntax and documentation are less
coherent.

Improvements:

- Provide one native function type representation used by aliases, fields,
  callbacks, hover, and completion.
- Keep LuaDoc function annotations interoperable with that representation.
- Delay generics until concrete container or identity-preserving APIs justify
  the additional language complexity.

### P2: Strengthen source maps

Improvements:

- Add golden mappings for defaults, classes, imports, compound assignment,
  runtime checks, sprites, and combinations of those features.
- Test multiline and non-ASCII source positions.
- Verify emitted runtime tracebacks against original Tua locations.

## Configuration And Project Model

### P2: Replace the handwritten partial TOML parser

[`config/mod.rs`](crates/tua-core/src/config/mod.rs) manually dispatches
sections and values, and string arrays are split by commas. This does not
fully implement TOML and makes every setting change cross-cutting.

Improvements:

- Use `serde` and `toml` with a typed configuration schema.
- Preserve defaults while reporting unknown keys with suggestions.
- Attach precise source ranges to configuration diagnostics.
- Generate VS Code settings and documentation from shared schema metadata
  where practical.

### P2: Improve file watching after correctness is fixed

Watch mode currently polls the source tree at a fixed interval and compares
length and modification time.

Improvements:

- Use a filesystem notification library with debouncing.
- Retain polling as a fallback for unsupported or unreliable filesystems.
- Content-hash ambiguous same-size/same-timestamp changes.
- Handle create, remove, rename, config, and asset events uniformly.

## Release And Documentation

### P2: Make generated LOVE data reproducible

Improvements:

- Pin the upstream LOVE API revision.
- Provide one documented generation command.
- Generate functions, overloads, types, hierarchy, enums, and tests together.
- Check generated output for drift in CI.
- Consider compact generated data instead of tens of thousands of Rust source
  lines if compile time or maintenance becomes a problem.

### P2: Establish a release pipeline

Improvements:

- Use one version source for Rust crates, the CLI, language server, and VS Code
  extension.
- Add crate descriptions, repository links, readme metadata, and changelog.
- Build signed or checksummed server binaries for supported platforms.
- Package the server with platform VSIX artifacts or use a trusted downloader
  with checksum verification.
- Document compatibility and migration notes for each release.

### P2: Restructure roadmap and specification documentation

The roadmap is an extensive implementation ledger with duplicate phase
numbers, while parts of the specification describe non-goals that have
already been implemented through LuaDoc or semantic analysis.

Improvements:

- Maintain a concise Now, Next, and Later product roadmap.
- Archive completed implementation phases separately.
- Keep an explicit language and editor capability matrix.
- Reconcile specification non-goals with current behavior.
- Move canonical specification content under the documentation site while
  preserving versioned historical copies when needed.

## Verification Baseline

Results from the 2026-07-15 audit:

- `cargo test -p tua-core`: 643 passed.
- `cargo test -p tua-lsp`: 186 passed.
- `cargo test --workspace`: failed in the bundled `love-full` example.
- `cargo clippy --workspace --all-targets -- -D warnings`: failed with 20
  diagnostics.
- `cargo fmt --all -- --check`: passed.
- `npm --prefix editors/vscode run check`: passed, with mocked coverage only.
- `make docs-build`: passed.
- Completion profile on the current medium fixture: p95 1.06 ms, but the
  fixture is too small and serial to represent reported editor contention.

## Recommended Execution Order

1. Fix LOVE inheritance/overload correctness and restore a passing workspace.
2. Make LSP work cancellable and remove forced suggestion races.
3. Add realistic concurrency benchmarks and real VS Code integration tests.
4. Replace completion project scans with incremental semantic indexes.
5. Fix watch-mode dependency invalidation.
6. Restore Clippy and introduce CI/release gates.
7. Consolidate plugin lowering, sprite metadata, and other domain intelligence
   through shared semantic queries.
8. Continue type-system, source-map, configuration, packaging, and
   documentation improvements after the editor correctness baseline is stable.
