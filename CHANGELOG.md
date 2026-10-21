# Changelog

All notable changes to Tua are documented here. Releases follow semantic
versioning while the project is pre-1.0: minor releases may deliberately evolve
language or editor behavior, and migration notes call out those changes.

## [Unreleased]

### Added

- Leaf sprite players can add their current frame and complete transform to a
  caller-owned `SpriteBatch` with `addToBatch`, enabling explicit bulk drawing
  without changing normal draw order or nested-sprite behavior.
- Unannotated Lua modules now retain richer inferred runtime surfaces through
  `require`, including root function modules, anonymous factory return shapes,
  multiple returns, nested table assignments, safe table aliases, conditional
  fields, and common `setmetatable` constructor instances.
- Opt-in `math` and `love.math` game helpers provide `tau`, interpolation,
  clamping, remapping, rounding, smoothing, sign, and byte-to-LOVE `rgb` and
  `rgba` color tables, with typed editor support and per-file emission only
  when referenced.

### Changed

- The bundled state-machine runtime now rejects unavailable events before
  building callback arguments, precomputes event-specific callback and async
  state keys once per transition closure, and caches per-state callback names.
  Handlers receive their arguments directly instead of through a packed table,
  and the transition phases recurse through a fixed-arity step, so LuaJIT can
  compile them. Transitions without extra arguments no longer allocate. In the
  included macOS arm64 LOVE 11.5.0 microbenchmark, a transition dropped from
  about 265 ns to 6.5 ns with the JIT on and from about 320 ns to 280 ns with
  it off. Extra arguments are now forwarded by count, so trailing `nil` values
  reach every callback, including after a deferred transition resumes. Valid
  synchronous, deferred, resumed, interrupted, and cancelled transition
  behavior is otherwise unchanged.
- Generated sprite sheets now share their precomputed marker-frame index across
  players, while child traversal uses an ordered instance array instead of
  repeating sheet-name lookups. Their normalized animations also clone frames,
  durations, and rebuilt intervals in one pass. Animation state and all public
  arrays remain independent between players, while custom sheets and replacement
  anim8 runtimes retain the public clone path. In the included macOS arm64 LOVE
  11.5 benchmark over 2,000 players, construction memory fell about 18%. With the
  JIT disabled, construction, nested updates, and leaf draws improved about 26%,
  13%, and 25%; with the JIT enabled, construction was about 1.5x faster,
  updates stayed near parity, and leaf draws improved about 40%. These figures
  are platform- and workload-specific; `make profile-sprite-love` compares
  another revision locally.
- Leaf sprites now pass their transform directly to anim8, avoiding redundant
  graphics push, translate, rotate, scale, shear, and pop calls. Nested sprites
  retain the graphics-stack path so children continue to inherit their parent
  transform. Fallback tables for custom, incomplete sheet structures are also
  reused, while compiler-generated sheets continue to provide their own.
- The bundled object-pool runtime now uses an explicit free-stack count and one
  identity-state table instead of Lua's table-length search plus separate
  ownership and activity maps. Warmed acquire/release cycles remain
  allocation-free while retaining foreign-object, duplicate-release, reset,
  prewarm, disposal, and capacity behavior.
- The bundled signal runtime now emits directly from a stable listener version
  instead of copying the listener list for every event. Subscribing or
  unsubscribing during an emit uses copy-on-write, so the active emit keeps its
  original listeners while nested and later emits see the updated version.
- The bundled audio runtime now caches each effective bus gain across an
  update, tracks the active voice count without building temporary lists, and
  publishes its control store only when the active count or music state
  changes. Idle `audio:update(dt)` calls no longer allocate per-frame bus
  cycle tables, active-voice lists, or mutator closures, and audio subscribers
  are no longer awakened every frame when their observable state is unchanged.
- The bundled store runtime no longer copies an entire state table for
  unobserved or watcher-only updates. Unobserved updates allocate no snapshots,
  watcher-only updates retain just the watched top-level values in a reused
  per-store table, so warmed watcher-only updates allocate nothing, and listener
  lists and watcher maps use immutable versions copied only when subscriptions
  change, including safely from inside a notification. In
  the included macOS arm64 Lua 5.4 microbenchmark, updates to a 2,000-field
  store were about 800x faster without observers and 110x faster with one
  watcher, while the required full-state snapshot kept the whole-store
  subscriber case near parity. These figures are platform- and
  workload-specific; `make profile-store` compares another revision locally.
  Because `update` now chooses its snapshot before the mutator runs, a
  subscriber or watcher added from inside a mutator is first notified by the
  next `set` or `update` rather than by the update that added it.
- The bundled ECS runtime iterates queries and systems without redundant
  per-entity liveness checks, wrapper closures, or temporary argument tables.
  Common query widths use specialized allocation-free loops, while wider
  queries reuse one argument buffer per iteration. In the included benchmark
  on macOS arm64 with LOVE 11.5.0 and LuaJIT 2.1, every measured workload ran
  at least 2.5× faster. The 10,000-entity four-component system improved about
  5.4× with JIT disabled and 20× with JIT forced on because it no longer
  creates roughly 2.3 MB of temporary tables per update. These figures are
  platform- and workload-specific. `make profile-ecs-love` compares against
  the latest release tag by default, or a selected `ECS_BASE_REF`.
- `Vec2` arithmetic whose operand types are known now emits as one
  constructor call instead of one BrineVector allocation per operator, with
  identical results including floating-point rounding. Fusion covers `+`, `-`,
  unary `-`, `*` by a number or `Vec2`, `/` by a nonzero number literal, and the
  matching compound assignments, when every repeated operand is a bare name,
  number expression, or `vec2(...)` call. Field and index reads such as
  `self.pos` and `positions[i]` are fused too when the expression is the whole
  right-hand side of an assignment, compound assignment, `local`, or `return`:
  each read is bound to a local exactly once, in evaluation order, inside a
  `do ... end` block. Comments inside the expression, deeper assignment
  targets, and anything else that may have observable evaluation keep
  BrineVector's operators. On macOS arm64 with LOVE 11.5.0 and the JIT
  disabled, `positions[i] = positions[i] + velocities[i] * dt` over 10,000
  vectors ran about 2.6× faster with half the garbage,
  `body.pos += body.vel * dt` about 2.3× faster, and a projectile step with
  three operators about 3.8× faster. With the JIT enabled, LuaJIT already
  removes the temporary vectors, and timings were unchanged. Project builds
  also fuse vector values inferred from required modules even when the
  consuming file never spells `vec2` or `Vec2`. Generated locals can appear in
  runtime error messages, and all hoisted reads occur before component
  arithmetic, so later side-effecting `__index` reads may run before an earlier
  invalid operand raises. These figures are platform- and workload-specific.

### Fixed

- Names followed by a comment on the same line, or preceded by a comment on
  the line above, now keep their types. Several analysis helpers read a token
  together with its surrounding comments, so `local sum = price + tax -- note`
  followed by `return sum` reported `TL1003: expected number, found nil`.
- A compound assignment after a comment line no longer copies that comment
  into its right-hand side (`x = -- comment` followed by `x + 1`).
- Compound assignments keep a binary right-hand side grouped: `x *= a + b` now
  emits `x = x * (a + b)` instead of `x = x * a + b`.
- Compound assignments on a type with an arithmetic metamethod now check the
  right-hand side against that operator, so `velocity *= 0.98` on a `Vec2` is
  accepted and keeps the `Vec2` type, while `velocity += 2` is still rejected.
- Unary minus on a `Vec2` now has type `Vec2`, using a new `__unm` metatable
  fact, instead of an unknown type.
- `Vec2` multiplication and division now reject operands other than a number or
  another `Vec2`, including `2 / velocity`, which BrineVector rejects at
  runtime. Plain `{ x, y }` tables no longer satisfy the built-in nominal
  `Vec2`; a project that declares its own `type Vec2` keeps it structural.
- Arithmetic metamethod operands are checked positionally, as Lua calls them:
  when the right operand supplies the metamethod, the left operand is checked
  against its first parameter.
- ECS query mutation now remains deterministic when `world:clear()` is called
  from a callback, and `world:add` rejects `nil` component values before they
  can corrupt component-store counts.
- ECS diagnostics now understand multiline calls, components exported from
  other modules, and inferred or annotated component-value variables. Typed
  pooled payloads no longer receive false missing-field or unknown-component
  errors, while incompatible values still report `TL5002`.
- Hover reference and implementation actions, including reference CodeLens
  clicks, now normalize VS Code and protocol-shaped locations before opening
  the references view, avoiding extension-host argument constraint failures.
- Editing a Lua module now compares its dependency surface using Lua source
  semantics, so implementation-only changes keep unchanged importers cached.
- Static ECS component schemas now flow through `world:get`, `world:add`, and
  `world:remove` across module exports, providing payload hover, completion,
  inlay hints, and field checking without repeated local annotations.
- Undefined-variable `TL4004` diagnostics are now errors in Tua sources, giving
  misspelled or undeclared names red editor and file decorations, while plain
  Lua sources retain warning severity for runtime-provided global compatibility.
- Required-module exports now preserve inferred built-in domain aliases such as
  `EcsEntityId`, `EcsWorld`, and `EcsComponent` in hover and inlay hints instead
  of degrading them to `any` or expanded structural types.
- Type-only imports referenced from anonymous function parameters and return
  annotations now resolve through project checking without leaving stale type
  mismatch diagnostics, while preserving imported table-field completion.
- Release checksum manifests now use portable LF line endings, and extension
  publishing tolerates older CRLF entries produced by Windows builders.

## [0.2.9] - 2026-10-16

### Added

- Opt-in `[[asset_pipelines]]` run standalone importers through a versioned
  manifest protocol, cache results by configuration and input content, surface
  source-linked diagnostics, materialize typed generated modules safely, and
  participate in `check`, `build`, and `watch`.
- Declaration-only `.d.tua` modules publish aliases with `export type` without
  producing Lua output. Namespace and named `import type` declarations plus
  named type re-exports support project diagnostics, navigation, type-context
  completion, cycle detection, and symbol-level cache invalidation.
- `--!tua generated` marks Lua 5.1-compatible generated `.tua` modules for
  direct parsing, while LuaDoc array contracts validate records incrementally
  without requiring a plain-Lua data module and handwritten typed facade.
- Static `spriteSheet({...})` configurations now derive exact animation and
  marker names, ordered multi-row frame ranges, registration origins, and a
  replaceable `SpritePlayer` runtime with named playback and manual seeking.
- Sprite animations can preserve per-frame trim/source metadata, and sheets
  can compose named child sheets through a declarative nested display tree.
- A compiler-aware `audioSystem({...})` helper provides typed buses, cues,
  effects, and layered music scenes; cached built-in audio analysis supplies
  envelopes and tempo metadata to generated catalogs and the VS Code Audio
  Explorer.
- `make release-version VERSION=<version>` now synchronizes release metadata,
  runs CI checks, creates the release commit, and adds an annotated local tag
  without pushing it.
- A provider-neutral, compiler-backed LLM language evaluation suite supplies 40
  executable tasks, compact prompt cards, reference and must-fail controls,
  JSONL model interchange, category reports, and an optional loopback-only
  Ollama lane. Trusted CI verifies the offline corpus without contacting a
  model.
- Representative LSP benchmarks now cover large single files, 100- and
  1,000-file projects, deep dependency chains, diamonds, and cycles. Separate
  large-table profiles measure compiler/LSP latency and peak memory.

### Changed

- The language server now derives project layout from the shared project model,
  routes requests to the active workspace in multi-root sessions, batches
  semantic warming, preserves runtime-module facade types across forwarding
  layers, and reserves request capacity so client follow-ups are not starved by
  background work.
- Restricted Mode launches the bundled server with explicitly contained
  execution and workspace behavior. Build outputs, generated assets, project
  loading, and external tool sessions now fail closed when paths escape their
  configured roots.
- Trusted CI and tagged release verification are separated from untrusted pull
  requests. Linux exercises the real LOVE debugger/runtime path, while macOS
  no longer bypasses Gatekeeper for an unsigned LOVE application bundle.
- The VS Code extension package is minimized, activates project UI only for Tua
  workspaces, and resolves bundled server binaries portably across supported
  platforms and checkout line endings.
- Performance trend collection now writes and uploads deterministic smoke/full
  reports, validates report freshness, and uses bounded shared-runner budgets
  for representative completion workloads.

### Fixed

- Large generated table modules now use lexical grammar selection instead of
  speculative parsing and panic recovery, class discovery uses indexed source
  positions, and inferred structural unions use hash-bucketed stable
  deduplication instead of repeated deep comparisons.
- Sprite previews now honor static nonuniform duration tables, and API/MCP
  asset metadata refreshes the workspace asset index before answering.
- Repeated version synchronization now preserves Cargo lockfile package
  boundaries before the release helper performs its verification pass.
- Restricted execution no longer permits unsafe server selection or filesystem
  traversal through build outputs, generated assets, configuration, API/MCP
  paths, or project discovery.
- Anonymous functions and named functions now enforce non-`nil` return paths
  consistently, while runtime module facades keep projected exports precise
  instead of forwarding an entire dynamic module surface.
- Compound assignments now reject unsupported computed/call targets and lower
  accepted name and field-chain targets structurally with single-evaluation
  semantics.
- Deep require graphs, type-only re-export chains, and dependency cycles use
  stack-safe traversal and retain precise editor behavior at representative
  project sizes.
- Polling file watching now bounds debounce batches and synchronizes readiness,
  preventing missed external asset-pipeline changes and Windows shutdown
  timeouts.
- LSP fixtures no longer scan the shared temporary directory, depend on host
  line endings, or assume Unix executable names, keeping local and hosted tests
  deterministic across Linux, macOS, and Windows.
- Parser, generated-table, and analysis paths now apply bounded depth, input,
  process, stderr, and memory-sensitive behavior; dependency and release
  automation were hardened alongside the documented security model.

## [0.2.8] - 2026-07-31

### Changed

- Extension registry publishing now starts only after the GitHub release
  workflow completes successfully, while retaining manual tag-based recovery.
- Version bumps now use `make set-version VERSION=<version>` to update the Rust
  workspace, Cargo lockfile, and VS Code package metadata together.

### Fixed

- Debug-server Make targets now select the platform executable suffix, so LSP
  smoke, profiling, ranking, and debugger commands invoke `tua.exe` on Windows.

## [0.2.7] - 2026-07-31

### Fixed

- LSP test fixtures now normalize module paths and derive declaration paths
  from file URIs, keeping completion and hover assertions portable on Windows.

## [0.2.6] - 2026-07-30

### Changed

- The VS Code extension is now published as `SolenodonteLabs.tua` on Visual
  Studio Marketplace and under the `SolenodonteLabs` Open VSX namespace.

## [0.2.5] - 2026-07-30

### Fixed

- Windows release tests now use the same analysis-thread stack size as the Tua
  CLI and language server, preventing debug AST walks from overflowing the
  smaller default Windows test-thread stack.
- File URI conversion now preserves valid separators and drive prefixes on
  Windows, handles UNC paths, and keeps virtual `tua:/` document identities
  stable across platforms.
- The embedded default `tua.toml` template now retains LF line endings on
  Windows checkouts, keeping generated configuration and schema drift checks
  deterministic.
- The mixed Tua/Lua LOVE showcase is included in release checkouts so its
  compiler and runtime integration test can run on every platform.
- The two-sample representative benchmark smoke gate now allows shared-runner
  variance without relaxing stale-result or semantic-validation failures.

## [0.2.0] - 2026-07-30

### Added

- An experimental compiler-backed **Tua: Preview State Machine** graph with
  initial-state emphasis, multi-source and wildcard routes, cycles, live edit
  refresh, multiple-machine selection, distinct theme-aware pastel state
  colors, obstacle-aware routing corridors, and source navigation without
  executing project code. Hover actions now link directly to state-machine,
  sprite-sheet, and recognized asset previews through explicitly trusted
  commands.
- Experimental, opt-in local Ollama inline completions with compiler-built FIM
  context, a Lua-first Tua/LOVE feature harness, inferred-local guidance,
  foreign-syntax rejection, stale-safe insertion-aware cloned-analysis
  validation, loopback-only transport, warming/status commands, cancellation,
  caching, and source-free counters.
- Read-only `tua api --stdio` JSON-RPC and `tua mcp --stdio` external-tool
  sessions, including model context/validation, explicit refresh and overlays,
  edit previews, a compact `llms.txt` contract, and twelve MCP tools.
- Suppressible class-design lints for child constructors that skip `super(...)`
  on a returning path (`TL4057`), overridable method calls during open-class
  initialization (`TL4058`), and redundant type-only inherited fields
  (`TL4059`). Always-on class contract failures remain `TL1008`.
- Resolved `---@deprecated` API guidance through standard lint `TL4056`,
  module exports, hover, completion tags, and deprecated semantic tokens.
- Standard lint rules for `tua lint` and the default-on
  `diagnostics.rules` editor/check setting, including symbol, declaration,
  control-flow, format-string, table-operation, and expression guidance.
- Source-mapped LOVE debugging through **Tua: Debug LOVE Project**. Builds now
  emit standard `*.lua.map` sidecars alongside Tua traceback maps, allowing the
  Second Local Lua Debugger extension to bind `.tua` breakpoints and show
  mapped Tua stack frames while debugging generated Lua. Game workspaces can
  start the same prepared workflow from `.vscode/launch.json` with
  `tuaProject: true`.
- A zero-setup Baton-backed `input({...})` helper with derived control and pair
  names, LOVE input-source completion and validation, name-dependent return
  signatures, active-device typing, and an on-demand bundled `baton.lua`
  runtime.
- An explicit zero-setup `store({...})` helper with inferred state reads,
  typed replacement and mutation, whole-store and keyed subscriptions,
  idempotent unsubscribe functions, and an on-demand `tuastore.lua` runtime.
- A zero-setup `stateMachine({...})` helper backed by lua-state-machine, with
  derived state/event unions, generated event methods, callback and value
  completion, and an on-demand bundled `tuastatemachine.lua` runtime.
- A zero-setup `ecs` module with a bundled data-oriented `tuaecs.lua` runtime,
  documented completion, hover, named signatures, world queries, and systems.
- A zero-setup `objectPool({...})` helper with item-aware typing, lifecycle
  completion and hover, prewarming, guarded release, cleanup hooks, and an
  on-demand bundled `tuaobjectpool.lua` runtime.
- A VS Code quick-pick and editor-toolbar command for Full, Compact, Types
  Only, Parameters Only, and Off inlay-hint modes. Compact restores the exact
  previous Tua-specific hint-length preference when another mode is selected.
- A compiler-backed `Explain Type / Why Is This Any?` command with source-linked
  annotation, inference, widening, narrowing, module, field, and dynamic
  fallback steps.
- Generated LOVE callback assistance with `TL5006` typo/signature diagnostics,
  typo rename fixes, and a source action for missing core lifecycle callbacks.
- Compiler-owned asset inventory and a VS Code Tua Assets Explorer grouped by
  type and static usage, with reference navigation, dimensions, refresh,
  reveal, and LSP-aware file rename.
- Asset path casing warning `TL2008` and conservative unused-asset hint
  `TL2009`; dynamic asset loading remains supported.
- A compiler-backed VS Code module dependency explorer for static `require` and
  `import` relationships, unresolved resolutions, cycles, reverse dependencies,
  exported surfaces, and source-linked "Why Is This Module Included?" paths.
- A compiler-backed VS Code ECS Explorer for project components, systems,
  queries, conservative callback read/write access, exact source navigation,
  and non-diagnostic allocation insights.
- A compiler-backed VS Code Object Pool Explorer for pool declarations,
  lifecycle operations, item types, conservative unreleased-acquire warnings,
  and source navigation, plus a quick fix for repeated zero-argument
  constructors.
- Conservative refactor assists for extracting selected expressions to locals
  or parameterized local functions, and inlining stable local bindings across
  resolved reads.
- A quick fix that generates a named Tua table type from stable local table
  literals and annotates the local binding in the same workspace edit.
- Exact-code quick fixes that suppress an eligible diagnostic for its line or
  file while preserving existing comments and respecting suppression settings;
  suppression actions are excluded from safe fix-all.
- SignalBus graph filters for event, bus, file, listener, and emitter views,
  plus direct schema-conflict display and actions to create typed listeners or
  declarations from compiler metadata.
- Shared VS Code webview, URI/range, error, and document-response utilities now
  own CSP nonces, safe JSON embedding, HTML escaping, location opening, and
  stale-response checks across editor domains.

### Changed

- Example profiling now measures intentional lint-diagnostic exits instead of
  aborting before JSON reports are written. CI workflows also use Node
  24-compatible GitHub action releases for checkout, setup, and artifacts. The
  reduced-sample representative completion smoke gate now allows 2.5 seconds
  for shared-runner contention while retaining its stale-result and response
  validation checks. Full performance-trend runs use a separate 30-second
  shared-runner ceiling, and their freshness check now uses a validated
  new-version completion as its protocol-ordering barrier.
- Model completion now distinguishes short local edits from function-body
  completion. Function contexts may generate and validate complete bodies up to
  64 lines instead of sharing the former five-line limit.
- Language-server request workers now use an explicit parser-safe stack budget,
  so large files remain responsive while functions and nested Lua blocks are
  temporarily incomplete during editing.
- Expected inline-completion cancellations and content-modified responses are
  now quiet. The extension also distinguishes Tua language-server failures from
  missing Ollama servers, models, and timeouts in its completion status.
- **Tua: Open Generated Lua Beside Tua** now saves the active source, rebuilds
  its project with the selected Tua executable, and reloads an already-open
  generated editor before showing the emitted Lua.
- Standalone `super(...)` calls remain constructor calls when preceded by
  comments; punctuation inside comment trivia no longer makes lowering treat
  them as member access.
- Native `T[]` array syntax now parses consistently in aliases, locals,
  parameters, returns, class fields, and class methods. The Luau-only `{ T }`
  array shorthand remains rejected with guidance to use Tua syntax. Indexed
  writes are checked against the array element contract, and assignment hover
  keeps the target index and value types distinct.
- A post-parse Tua dialect validator now rejects Luau-only grammar as blocking
  `TL0004` before semantic analysis or emission, with targeted remediation for
  constructs such as `::` type assertions and `continue`. Successful emission
  still includes a final Lua 5.1 parse, with `TL2010` reserved for compiler
  output that violates the target-language invariant.
- `TL1003` now reports structured nested field, dictionary, tuple, callback
  parameter, and callback-return mismatch paths. Table-literal failures
  highlight the offending leaf value, while root mismatches retain concise
  declared type names.
- Hover now resolves nested built-in helper fields and callback parameters.
  Definition opens virtual schemas for fixed configuration fields and follows
  source-derived store keys, sprite animation names, state-machine events, and
  callback names back to their originating entries.
- Record-shaped tables with compatible named string fields now satisfy
  string-keyed dictionary types, including static `spriteSheet` animation maps.
- Class names, fields, and methods now support source-accurate definition,
  references, and project-wide rename, including nominal instances exported
  through static modules. Resolved named table-shape fields are also renameable.
- Class inheritance now preserves field contracts: redeclarations must keep the
  resolved parent type, and non-`nil` fields must be initialized on every
  constructor path through an assignment, default, or guaranteed parent
  initializer.
- Static module exports now preserve nominal class identity and inheritance, so
  structurally identical classes remain distinct across `require` and `import`
  boundaries.
- Classes may now extend a parent declared later in the file. Lowering keeps a
  stable forward parent table, so inherited methods and `super` calls work once
  the parent declaration has executed.
- Class constructors now forward typed varargs and preserve every explicit
  `super(...)` argument, while member calls such as `helper.super()` remain
  ordinary Lua calls. Subclasses may declare constructor signatures independent
  of their parents; each `super(...)` call is still checked against the parent
  initializer.
- Optimized class constructors now preserve explicit constructor semantics by
  applying each class's `init` assignments before its field defaults.
- Class parsing now preserves class-like text in Lua long-bracket strings and
  comments, reports `TL1008` for unrecognized members, and accepts table-shaped
  method return types. Checker diagnostics produced from generated class bodies
  now map back to the original class declaration instead of generated Lua lines.
- Exact-code `tua-ignore-line`, `tua-ignore-next-line`, and header-only
  `tua-ignore-file` suppressions are now enabled by default. Empty directives
  and broad category suppressions report `TL4025`, while Luau `--!nolint`
  guidance points to Tua's file form.
- `MisleadingAndOr` now identifies the exact `false` or `nil` alternative,
  points at the complete expression, and recommends a LuaJIT-safe `if`
  statement assignment instead of Luau-only conditional-expression syntax.
- `ComparisonPrecedence` now exempts symmetric negation, reports the actual
  operators with inverse-equality or explicit-`and` guidance, and highlights
  the complete ambiguous expression.
- `DuplicateCondition` now structurally compares expressions, flattens grouped
  boolean chains, preserves runtime literal identity, suppresses the common Lua
  and-or ternary idiom, and points diagnostics at the repeated condition with
  its origin.
- `TableOperations` now resolves Lua builtins, detects expanding final
  `table.insert` calls, and uses shared table types for conservative `#` and
  `ipairs` guidance, including `__len` and `__ipairs` support.
- Native standalone dictionary annotations such as `{ [string]: Value }` now
  lower to dictionary types instead of losing their index signature.
- `DuplicateLocal` now reports repeated local and parameter names uniformly as
  `TL4043`, recognizes implicit method `self`, identifies the original
  declaration, and accepts repeated `_` placeholders.
- `ImportUnused` now recognizes dynamic calls to the global `require`, excludes
  lexically shadowed calls, counts qualified type references, includes `_`
  guidance, and backs import source actions with the same resolved usage facts.
- `FunctionUnused` now reports direct local and user-global function
  declarations as `TL4055`, with safe editor removal for complete declarations.
- Standard lint rules are enabled by default for `tua check` and editor
  diagnostics. Set `diagnostics.rules = false` to opt out; `tua lint` continues
  to enable the standard set for its own run.
- `UnknownGlobal` now recognizes explicit global assignments and simple global
  function declarations anywhere in the current Tua file, regardless of source
  order.
- Formatting and linting are now independent toolchain modules: `tua fmt` and
  `fmt --check` own canonical layout, while `tua lint` reports syntax/lint
  categories and `tua check` owns the complete semantic diagnostic pipeline.
- ECS analysis is enabled by default and can still be disabled through
  `[ecs].enabled` or the matching editor setting.
- VS Code syntax highlighting now covers the full Lua/Tua and game-helper
  surface, while semantic highlighting uses standard theme-compatible symbol
  types and declaration, readonly, deprecated, modification, and library
  modifiers.
- The VS Code extension now requires VS Code 1.94 or newer for native compact
  inlay-hint presentation.

### Migration

- Replace empty or category-wide inline ignores with explicit `TLxxxx` codes.
  Projects that forbid source suppressions can set
  `[diagnostics].inline_ignores = false`.
- Replace `TL4008` suppressions or filters with `TL4043`; the
  `duplicate_parameters` setting remains available and now emits `TL4043`.
- Projects that need the previous lint-free `tua check` and editor behavior can
  set `[diagnostics].rules = false`.
- Remove `diagnostics.formatting_lints` and the
  `tua.diagnostics.formattingLints` editor override. Use `tua fmt --check` for
  formatting enforcement and `diagnostics.rules` for the standard lint rules.

## [0.1.0] - 2026-07-16

### Added

- Typed Lua parsing, checking, readable Lua 5.1/LuaJIT emission, source maps,
  formatting, linting, watch builds, and project-aware module analysis.
- A preemptible language server with completion, hover, signature help,
  navigation, refactors, semantic tokens, inlay hints, diagnostics, and LOVE
  catalog intelligence.
- LOVE asset helpers, generated LOVE 11.x API data, Vec2, SignalBus, ECS, and
  preview-gated anim8-backed sprite-sheet support.
- Platform-specific VSIX packages containing matching Tua server binaries and
  standalone server archives with SHA-256 checksums.

### Compatibility

- Tua 0.1 targets Lua 5.1-style Lua, LuaJIT, and LOVE 11.x.
- The CLI, language server, Rust crates, and VS Code extension now share version
  `0.1.0` from the Rust workspace metadata.

### Migration

- Replace any development VSIX named `tua-0.0.1.vsix` with the VSIX matching
  your operating system and architecture.
- Project source and `tua.toml` files require no migration for this coordinated
  release.

[Unreleased]: https://github.com/Kyonru/tua-lang/compare/v0.2.9...HEAD
[0.2.9]: https://github.com/Kyonru/tua-lang/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/Kyonru/tua-lang/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/Kyonru/tua-lang/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/Kyonru/tua-lang/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/Kyonru/tua-lang/compare/v0.2.0...v0.2.5
[0.2.0]: https://github.com/Kyonru/tua-lang/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Kyonru/tua-lang/releases/tag/v0.1.0
