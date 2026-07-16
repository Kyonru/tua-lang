# Tua Improvement Backlog

Repository-wide audit performed on 2026-07-15. Priorities reflect user impact,
correctness risk, architectural leverage, and release readiness.

## Highest Priority

### P0: Fix the bundled LOVE project regression

Status: done on 2026-07-15.

The bundled `love-full` regression was fixed through shared catalog data:

- LOVE object supertypes now participate in nominal assignability, including
  `Image` to `Drawable`.
- Call checking now selects from the same generated overload variants used by
  signature help, including `love.graphics.newFont(path, size)`.
- Functions, overloads, enums, object methods, and type hierarchy are generated
  from pinned `love-api` revision
  `4b2d89510eabd320791c4a8157e6e6656c34ec09`.
- CI runs the workspace and bundled LOVE tests and checks generated catalog
  drift.

Regenerate or verify the catalog with `make generate-love-api` and
`make love-api-drift-check`, passing `LOVE_API_ROOT=/path/to/love-api`.

### P0: Make LSP work preemptible

Status: done on 2026-07-15.

The stdio coordinator now keeps transport, document synchronization, revision
tracking, and cancellation responsive while analysis requests run in separate
interactive and background worker lanes:

- Completion, signature help, hover, definition, and other interactive queries
  cannot be queued behind diagnostics, inlay hints, semantic tokens, or symbols.
- Every worker request captures its document generation and owns a cancellation
  token. `$/cancelRequest` returns `RequestCancelled`; document changes cancel
  stale background work and stale foreground results return `ContentModified`.
- Identical document diagnostic, inlay, semantic-token, symbol, and other
  background requests at the same revision share one worker job.
- Debounced full push diagnostics run on the background lane rather than the
  transport loop.
- Transport-level tests cover typing during a running slow request, stale-result
  rejection, explicit cancellation, and duplicate request coalescing.

Cancellation is cooperative at the scheduler boundary. Queued work is skipped
and completed stale work is discarded; a core query already executing may run
to completion on its worker before its result is dropped.

### P0: Remove the extension suggestion race

Status: done on 2026-07-15.

The extension now relies exclusively on VS Code's language-scoped zero-delay
quick suggestions and the normal LSP completion provider. The fixed 25 ms edit
listener, forced `editor.action.triggerSuggest` command, and its edit-debug
helpers were removed, so one edit state no longer receives a native request and
a second extension-forced request.

`make extension-test` launches a real isolated VS Code Extension Development
Host with a controlled LSP server. It types through an incomplete completion
list and asserts the synchronized document version, cursor position, one
request for the latest edit state, fresh replacement results, and acceptance
from the visible suggestion widget.

## Core Architecture

### P1: Replace per-request project scans with incremental indexes

Status: done on 2026-07-15.

The analysis file cache now stores compact member usage, receiver/member call,
symbol proximity, and semantically validated SignalBus event summaries. Only a
changed file rebuilds its summary; snapshots aggregate the summaries into a
revisioned project usage index without retaining or rescanning project source
text.

Project-aware completion ranking, SignalBus event-name completion, and the
signal graph consume that shared index. Completion cache keys include the
snapshot revision and semantic context, so an old snapshot cannot publish a
result into a newer revision. Ranking remains a secondary operation over
compiler-generated, visible candidates and cannot introduce an out-of-scope
member. `tua/cacheStats` and completion profiling expose usage-index cache
hits and misses, with regression coverage for changed-file invalidation,
ranking refresh, event refresh, and stale completion-cache isolation.

### P1: Make watch builds dependency-aware

Status: done on 2026-07-15.

Watch mode now owns one long-lived project `AnalysisHost`. Each debounced batch
applies semantic additions, edits, and removals to that host; filesystem rename
pairs are handled as one removal plus one addition. When an exported module
surface changes, watch mode unions the old and new transitive reverse dependency
closures, then rechecks and re-emits valid affected Tua files. Changed assets
continue through the existing copy and collision paths.

Project-aware snapshot diagnostics now run the checker with the cached semantic
environment for files with module edges, so an unchanged importer can report a
new argument or assignment error after an exported signature changes. Core and
CLI regressions cover transitive invalidation and this unchanged-importer case.
Polling precision and native filesystem notifications remain tracked under the
separate P2 watcher item.

### P1: Make ECS and sprite first-class domains

Status: done on 2026-07-15.

The experimental registry and validation trait were removed. They only wrapped
two built-in diagnostic calls and added dynamic dispatch, snapshot state,
replacement rules, public API, tests, and documentation without enabling an
external extension workflow.

Sprite and ECS validation now run directly while an `AnalysisFile` is built, so
their results retain the normal file/revision cache. Sprite validation consumes
parser-owned declarations and indexed asset metadata. ECS validation runs only
when `[ecs].enabled` is active. Separate `analysis.sprite.diagnostics` and
`analysis.ecs.diagnostics` profiler spans make their costs reportable.

`spriteSheet({...})` remains ordinary parser-native Tua with core-owned typing,
completion, hover, diagnostics, and anim8-backed emission. ECS typing,
completion, hover, signature help, and diagnostics are likewise core-owned.
Stable domain diagnostic codes remain in `TL5000`-`TL5999`; no executable
extension API is exposed.

A 10-run release profile over `examples/features` measured combined domain
validation at 0.4126 ms per check through the direct path versus 0.4316 ms
through the removed registry. The 4.4% stage-local reduction is only 0.019 ms
in absolute terms; reduced architecture and snapshot state are the primary
benefits, not a user-visible latency claim.

### P1: Introduce structured lowering

Status: done. The provenance boundary, typed-operation IR, parser normalization,
and declaration-level annotation slices landed on 2026-07-15.

`tua-core` now lowers through an ordered `LoweringDocument`. Every emitted line
has a source span and a `LoweringKind`, and the final `LuaEmission` artifact
contains both serialized Lua and generated-line provenance. Default parameters,
built-ins, imports, runtime checks, classes, final class calls, compound
assignments, sprites, and LuaCats annotations all flow through that boundary.
Serialization performs the final boundary normalization once after lowering.

`LoweringOperation` is now the internal lowering IR for byte-range insertions
and replacements. Operations carry their source span and lowering kind, apply
in deterministic order, and reject invalid ranges, overlapping replacements,
and offsets that split UTF-8 text. Default parameters, built-ins, imports,
runtime checks, final class calls, compound assignments, and sprites emit
operations directly instead of serializing and reconciling whole documents.
LuaCats aliases and declaration comments are insertion operations tied to their
typed declarations or module return statement.

Class expansion is applied by the lowering layer before the Luau parser and
records generated parser-line ownership at that boundary. Initial emission
provenance is reconstructed from surviving positioned syntax tokens aligned to
the canonical erased-AST serialization. This removed generated-class marker
recognition, skipped-source-line scanning, and the erased-Lua line bootstrap.

CLI builds now write source maps directly from `LuaEmission` provenance. The
old CLI-side parser, skipped-line reconstruction, and helper-specific mapping
heuristics were removed. Mixed golden output and end-to-end traceback tests
cover defaults, bundled helper requires, runtime checks, compound assignments,
classes, and sprites. Additional IR tests cover multiline replacements,
non-ASCII source, class-following source positions, declaration-level LuaCats
provenance, operation conflicts, and deterministic same-offset insertions.

Lowering is core-owned. There is no extension lowering API that can overlap
core transformations or bypass stable operation ownership and pass ordering.

### P1: Split semantic analysis into stable query boundaries

Status: done. The query-boundary split landed on 2026-07-16.

The analysis domain now separates host mutation and snapshot construction,
snapshot accessors, database/cache construction, degraded-source recovery,
editor queries, navigation queries, and project queries into focused modules
under [`analysis`](crates/tua-core/src/analysis). `analysis/mod.rs` owns the
shared state and helper model instead of every query implementation.

Every successfully parsed cached file now owns one mandatory semantic bundle:
HIR, scope map, and type environment. Type-checking diagnostics do not remove
that bundle. Project-aware semantic facts are derived lazily from it and cached
once per snapshot revision.

Temporarily invalid source uses one explicit degraded path. A cached file stores
the lossy parse, HIR, and base environment once; snapshot queries enrich and
cache those facts once for hover, completion, definition, signature help,
inlay hints, and semantic tokens. Queries no longer recurse from a degraded
parse into the valid semantic cache. Feature-specific lexical compatibility
fallbacks are confined to the recovery module and run only when no degraded
semantic parse can be built.

Dependency surfaces are tracked separately for require targets, module exports,
and visible type facts. Export or type changes invalidate the transitive
importer closure. Require-target-only changes rebuild the changed file and
revision-level project indexes without evicting importer semantic products.
Cache statistics expose a counter for each surface.

## Quality And Delivery

### P1: Add CI and restore strict linting

Status: done on 2026-07-16.

The required CI workflow now separates strict quality checks, Linux/macOS/
Windows workspace and example builds, VS Code extension checks and extension-
host tests, generated LOVE catalog drift, and a deterministic benchmark smoke
lane. Bundled `love-minimal`, `love-full`, `features`, and `project-model`
builds are required on every supported runner rather than being treated as
optional samples.

`cargo clippy --workspace --all-targets -- -D warnings` is clean. The completion
lint fixes replaced broad argument lists with ranking and field-context inputs;
the remaining core and LSP fixes use idiomatic references, iterators, paths, and
configuration construction without lint suppression.

`make verify` now uses the non-mutating `fmt-check` target. `make ci-check`
provides the local subset that does not need a pinned external LOVE API checkout
or a graphical VS Code host. Extension checks syntax-check every JavaScript
module, and JSON validation discovers every tracked `.json` file through Git.

`make benchmark-smoke` checks completion order, runs one examples timing pass,
and enforces semantic freshness and a generous completion ceiling on the
CI-sized representative fixtures. The separate performance workflow runs
`make profile-all` on `main`, weekly, or on demand and retains the LSP,
completion, representative, and bundled-example JSON reports as 90-day trend
artifacts.

### P1: Use representative performance benchmarks

Status: done on 2026-07-16.

[`generate-lsp-benchmarks.js`](scripts/generate-lsp-benchmarks.js) creates
deterministic exact 1,000-line and 5,000-line single-file fixtures plus 100-file
and 1,000-file projects under ignored `target/` storage. A hashed manifest and
generator tests verify file counts, line counts, and reproducible content
without checking 1,000 generated files into Git.

[`lsp-representative-profile.js`](editors/vscode/scripts/lsp-representative-profile.js)
starts a fresh server for every cold feature sample, records repeated warm
completion, hover, definition, and inlay-hint samples, and simulates editor
contention with 32 background requests followed by `didChange` and completion.
The JSON report includes p50/p95/p99, request age, cache hit rates, cancellation
rate, old-version results after the change, stale-result count, and semantic
probe failures.

Required CI profiles the 1,000-line and 100-file fixtures with a generous
2-second completion p95 gate. It also rejects stale results and invalid
semantic responses while recording cancellation behavior. The scheduled
performance lane profiles all four scales with a 10-second gate and retains
reports for 90-day trend analysis. On the implementation baseline, the
5,000-line contention probe exposed a roughly 5.1-second completion while warm
completion remained under 10ms, making the remaining preemption cost explicit
rather than hiding it behind the old 149-line serial profiler.

### P1: Add real VS Code integration tests

Status: done on 2026-07-16.

The extension check in [`package.json`](editors/vscode/package.json) validates
every JavaScript module, the manifest, and mocked activation. The required
`@vscode/test-electron` suite now launches an isolated two-root workspace
against a deterministic mock LSP and covers:

- one-character automatic suggestions, rapid edits, cancellation, late stale
  responses, synchronized document versions and UTF-16 cursor positions;
- visible suggestion acceptance without stale replacement;
- hover, Cmd+click-equivalent definition navigation, signature help, and inlay
  hints immediately after unsaved typing;
- untitled Tua documents and intelligence in the second workspace root;
- `workspace/willRenameFiles` edits plus `workspace/didRenameFiles`;
- sprite hover command links, preview opening, singleton/top-most reveal,
  reload from updated source, and missing-asset states.

The mock server records timestamped request, response, cancellation, document,
workspace, and file-operation events. Tests assert current document versions
and positions instead of depending on fixed sleeps. CI runs the extension host
under Xvfb after the fast syntax, manifest, and mocked-activation checks.

### P1: Split oversized test modules

Status: done on 2026-07-16.

The server test suite now lives under
[`server/tests`](crates/tua-lsp/src/server/tests). Completion, hover,
navigation, diagnostics, workspace, formatting, semantic tokens, inlay hints,
and scheduling regressions have dedicated modules. Smaller modules cover code
actions, document features, signature help, protocol behavior, and degraded
snapshot recovery.

Shared JSON-RPC encoding, the in-process server harness, temporary project
fixtures, request builders, and response assertions remain single-sourced in
[`mod.rs`](crates/tua-lsp/src/server/tests/mod.rs). All 192 existing server
regression tests were preserved during the split.

## Extension And LOVE Developer Experience

### P2: Move sprite semantics into the language server

Status: done on 2026-07-16.

`tua-core` now exposes structured sprite inspection from cached parsed
declarations and asset metadata. The general `tua/inspect` LSP request returns
document version and analysis revision metadata, declaration ranges, resolved
asset URIs/status, frame and animation values, and the same sprite diagnostics
used by builds and normal editor diagnostics.

The VS Code extension no longer parses `spriteSheet` tables, searches for
`tua.toml`, probes asset candidates, or derives source-backed sprite facts.
Its preview implementation is split into controller, inspector client, panel
state, view model, webview template, and webview runtime modules. Missing
assets, incomplete frame/animation configuration, invalid ranges, and duplicate
animation names are rendered from `TL5004` inspection diagnostics.

### P2: Expand LOVE-focused semantic support

Status: done on 2026-07-16.

LOVE object inheritance and function overload checking use generated type
supertypes and signature variants from the pinned `love-api` revision. Validated
asset literals now resolve definitions directly to asset files, while
references compare cached asset metadata across project files without
rescanning source.

Top-level callback identity is generated from the same catalog. Every cataloged
callback can produce a declaration skeleton, and completion resolve includes
generated callback parameter names, types, and documentation. `love.conf(t)`
keeps typed nested field completion and now validates the documented
`t.window.fullscreentype` literal values.

Generated Lua tracebacks already map through lowering provenance with
`tua traceback`. LOVE items, overloads, parameter docs, enum values, object
methods, callback identity, and inheritance remain reproducible through
`make generate-love-api` and guarded by `make love-api-drift-check`.

### P2: Complete SignalBus intelligence

Status: done on 2026-07-16.

The revisioned project usage index now owns one semantic SignalBus event index
keyed by compiler-confirmed bus identity and event name. Local buses use their
lexical declaration identity; required aliases resolve to the required module
identity so typed shared buses link across files. Same-named events on
independent buses no longer merge in navigation or `tua/signalGraph`.

Typed `on`, `once`, `register`, and `declare` callbacks contribute payload
schemas. Conflicting typed schemas publish `TL5005` at every conflicting event
definition. Untyped callbacks and unresolved/dynamic receivers remain
permissive.

Event-name completion, signature help, hover, definition, references, rename,
and the graph consume the shared index. Completion ranking is deterministic and
prefers the same bus, same file, declaration/schema calls, proximity, project
usage, and stable source order. Event rename edits only string contents, so
dotted event names remain ordinary Lua strings with no new syntax.

## Language And Type System

### P2: Define inferred variable reassignment semantics

Status: done on 2026-07-16.

Unannotated `.tua` locals now receive a stable widened contract from their
declaration initializer. Compatible simple and compound assignments preserve
that contract; incompatible assignments publish `TL1003`. Contracts are
lexical, survive control-flow joins, and remain separate for shadowed locals.

`nil` initialization stays `nil`, so delayed initialization uses an explicit
optional annotation. Explicit `any` and unresolved dynamic expressions remain
permissive. Ordinary `.lua` files retain dynamic reassignment unless LuaDoc
annotations constrain the binding. Hover and other semantic queries keep the
declaration contract while existing guard-based narrowing remains local to the
guarded flow.

### P2: Prioritize native function types before generics

Status: done on 2026-07-16.

Native callback types now use `(parameters) -> returns` across aliases, table
fields, local callback annotations, module exports, checking, hover,
completion, inlay hints, document symbols, and signature help. Parameter names,
typed varargs, and multiple returns survive the shared semantic representation.

LuaDoc/LuaCATS `fun(...)` types lower to the same representation, and LuaCATS
emission serializes native callback aliases back to compatible `fun(...)`
annotations. Generic function syntax and solving remain deliberately deferred
until a concrete container or identity-preserving API justifies the added
language complexity.

### P2: Strengthen source maps

Status: in progress. Source maps now consume compiler-owned lowering
provenance, and a mixed-transform traceback regression is covered.

Improvements:

- Add remaining focused golden mappings for imports and annotation emission;
  defaults, classes, compound assignment, runtime checks, sprites, bundled
  helper requires, and mixed transformations are covered.
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

Status: core reproducibility work is done. Remaining improvement:

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

Results from 2026-07-16 after restoring the delivery gates:

- `cargo test --workspace`: passed, including 666 `tua-core` tests, 193
  `tua-lsp` tests, CLI integration, Lua syntax, and bundled LOVE regressions.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `make build-examples`: passed for all four bundled projects.
- `npm --prefix editors/vscode run check`: passed for all 20 JavaScript files,
  manifest validation, and mocked activation.
- `npm run test:vscode`: passed in a real VS Code extension host.
- `make benchmark-smoke`, `make docs-build`, and `make json-check`: passed.
- A one-pass LSP profile wrote all small, medium, and large fixture metrics to
  `target/tua-speed/lsp-profile.json`.

## Recommended Execution Order

1. Fix LOVE inheritance/overload correctness and restore a passing workspace.
2. Make LSP work cancellable and remove forced suggestion races.
3. Add realistic concurrency benchmarks and real VS Code integration tests.
4. Replace completion project scans with incremental semantic indexes.
5. Fix watch-mode dependency invalidation.
6. Expand the restored CI baseline into signed, checksummed release artifacts.
7. Consolidate sprite metadata and other domain intelligence through shared
   semantic queries and keep lowering on the compiler-owned operation IR.
8. Continue type-system, source-map, configuration, packaging, and
   documentation improvements after the editor correctness baseline is stable.
