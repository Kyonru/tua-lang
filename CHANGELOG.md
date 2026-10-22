# Changelog

All notable changes to Tua are documented here. Releases follow semantic
versioning while the project is pre-1.0: minor releases may deliberately evolve
language or editor behavior, and migration notes call out those changes.

## [Unreleased]

### Fixed

- Calls after the first in a chain are now checked. Misuse such as
  `builder():add("two")` or `love.graphics.newImage(path):setFilter("bogus")`
  was previously accepted, and the chain took the first call's result, so
  `newImage(path):getWidth()` was typed as an `Image` instead of a `number`.
  An unknown link still leaves the rest of the chain dynamic.
- Multi-line LuaCATS aliases (a bare `---@alias Name` followed by
  `---| member` lines) now declare their union. They were previously dropped,
  so every value typed with them became `any`.
- Literal parameter types such as `---@param mode "fill" | "line"` now keep
  their contract when the function is used from another module. Inferred
  widening previously turned them into `string`, so a misspelled mode passed
  checking.
- Fixed method calls on the result of a required module function, such as
  `tween.to(box, 1, { x = 10 }):ease("quadout")`, reporting a false argument
  count error. The checker treated the inner `to(...)` as a `:` call and
  counted the module table as an extra `self` argument.
- VS Code Extension Host launch profiles now build the executed extension
  bundle before launch, emit source maps for JavaScript breakpoints, and stage
  the bundled server through a cross-platform Node script that selects `tua`
  or `tua.exe`. Normal F5 launch no longer runs the full extension check. A
  launch-config smoke check prevents missing tasks, stale `src` out-file
  mappings, and broken Unix or Windows staging from returning.
- Fixed the Phase 3 production evaluator retaining target-file scope facts from
  text after a simulated cursor. Every scenario now queries with the same cold
  fallback available during forward typing, and the evaluator no longer applies
  a second tune-only filter after the production predictor returns.
- Statistical inline completion no longer copies warmed analysis into the
  semantic last-good cache or changes hover/menu recovery behavior. Scope facts
  use original Tua coordinates, remain independently bounded, and degrade bare
  identifiers after a changed boundary to slots instead of abstaining. Decoder
  overflow, cancellation, and deadline exhaustion now fail closed; incomplete
  candidates are not returned.
- Statistical-only corpus builds no longer retain Phase 2 exact continuation
  tables, callback data, or Lua-file tables. Query caps select Tua evidence in
  deterministic path order, independent of background adoption order, and
  two-file support is required at the context order that chooses each token.
- The VS Code integration test API now requires both Extension Host test mode
  and an explicit test environment opt-in. Extension Host coverage also checks
  that the client declares its combined inline provider during initialization.

- Fixed the inline-completion evaluator loading manifest project directories
  through a file-oriented config search. Configured source and output roots are
  now honored, so generated Lua no longer leaks into tuning corpora or
  duplicates its Tua source. Corrected Phase 1–3 tuning results replace the
  contaminated reports; held-out and external sets remain unread.
- Deterministic inline completion now refuses files above its parse cap
  before scanning them for recall, instead of scanning up to 256 KiB and then
  discarding the result at validation. Refusals are counted as
  `skippedOversized` in LSP statistics and evaluator reports, and the size
  limit is documented.
- The Phase 1 gate now binds evidence to the code it measured. Representative,
  diagnostics-comparison, completion, editor-replay, and evaluator reports
  record their commit and dirty state, and each passed prerequisite names its
  artifact and revision. The gate check rejects evidence from a dirty tree or
  from before a later change to shipped code, harnesses, or benchmark inputs.
  Earlier evidence carried no revision, so those prerequisites are pending
  until rerun.
- Each Phase 1 prerequisite is now decided by a named check in its report
  rather than by the whole report passing. Completion performance uses a new
  `make profile-completion-phase1-gate` with exactly 5% tolerance instead of
  the ordinary profile's 1 ms floor. A new `inline_contended_delivery`
  prerequisite requires contended request age within the extension's 250 ms
  timeout. The validation parse distribution is now read from the evaluator's
  tuning run of the production host rather than a handful of representative
  requests, and the menu-replay and cache-hit evidence state what they do and
  do not show.
- Phase 2 project retrieval is parked. With corrected tuning data it adds no
  suggestion over Phase 1, so its held-out data stays unspent and its code
  stays behind the hidden switch, doing no work while disabled. The plan
  records what would unpark it and when to remove it instead. Its production
  evaluator now resolves modules with the real loaded project model, matching
  the LSP, and the conservative candidate keeps directly required modules even
  when they share the active file's directory. Required-module evidence is
  reported separately and cannot be diluted by unrelated project matches.

### Added

- Added a production-safe, default-off Phase 3 statistical inline-completion
  source for Tua. It reuses the immutable project corpus and speculative
  request lane and adds bounded token tables and compact scope facts. When
  statistical completion is enabled without hidden Phase 2 exact retrieval,
  corpus builds omit original-line continuation tables. Matching cached scope
  facts improve identifier classification; cold files and changed suffixes use
  conservative bound slots without triggering analysis. The predictor requires
  two distinct files at the deciding context order to support each token and passes
  candidates through the existing deadline, cancellation, VFS freshness,
  suffix, and parse-delta checks. The extension exposes it as the explicitly
  experimental `tua.completion.inline.sources.statistical` setting. Corrected
  leakage-safe tune-only production-path evidence improves combined
  correct-prefix precision from 0.728 to 0.735 with zero incorrect statistical
  characters, but only six scenario suggestions at three independent cursors,
  so it remains off by default and no held-out or external set was spent. A
  statistical-on representative LSP profile passed every resource/contention
  check with no stale results and at most 1.348 ms core p95 on fixtures where
  the predictor ran.
- Added standard LSP 3.18 `textDocument/inlineCompletion` support as an adapter
  over the same core query and newest-wins speculative lane used by Tua's
  custom request. Standard clients receive portable ranges and snippet
  `StringValue`s without internal source or ranking metadata. The Tua VS Code
  extension declares its combined local/Ollama provider during initialization,
  so the server suppresses duplicate standard-provider registration for that
  client. The VS Code
  status now lists enabled deterministic sources, and local observational
  counters migrate to v2 with separate project-statistical returned/accepted
  counts; they store no source text and do not affect ranking.

- Added a reproducible Phase 1 diagnostic-freshness comparison for
  deterministic inline completion. The representative harness can run a
  pre-Phase-1 binary in compatibility mode, alternates it with the
  inline-enabled candidate on identical generated fixtures, disables
  asymmetric internal profiling, and gates pooled p95 latency from 100
  content-bound edits per fixture. The comparator fails closed on fixture,
  workload, sample-count, or inline-coverage drift.
- Added the original Phase 3 evaluator experiment behind tua-core's
  `test-fixture` feature. Its raw and held-out model machinery remains
  evaluator-only and uses immutable target-file-held-out models,
  collision-free token keys, alpha-renamed identifier slots, bounded
  Jelinek-Mercer decoding, and a tested token renderer under explicit source,
  context, memory, cancellation, and deadline limits. New tuning and smoke Make
  targets compare the raw predictor, a pinned Tua-only high-confidence
  candidate, its incremental contribution after Phase 2 abstains, and a Phase
  2-first fallback. Reports add independent cursor counts that collapse
  scenario variants for the minimum evidence floor while leaving quality
  metrics scenario-weighted. A fail-closed, paired Phase 2-versus-combined gate
  contract now pins languages, contexts, score, incremental quality, utility
  gain, regressions, and latency before any frozen data is read. The checker is
  wired into the future gate branch and rejects eligible enabled-language data
  without a matching contract. The command accepts only named
  manifest tune/smoke sets and hard-refuses held-out, external, and ad-hoc paths.
  The default-off production source described above reuses the model primitives
  through the bounded host query instead of compiling the evaluator's held-out
  views into the LSP or extension. A separate committed synthetic smoke set now
  crosses the pinned confidence filter and runs under `benchmark-smoke`; its
  Make check
  and a focused assertion fail if the filtered source becomes a no-op or if
  any synthetic suggestion stops being an exact continuation, without
  changing the Phase 1 smoke baseline.

- VS Code now exposes **Tua: Toggle Deterministic Inline Completions** for the
  existing default-off workspace gate. Its inline-completion status reports
  `Local`, `Ollama`, `Local + Ollama`, or `off` instead of presenting every
  mode as Ollama. Source-free deterministic counters separately record items
  returned and accepted for semantic and current-file sources; the counters
  store no completion text, are persisted in 30-second batches with shutdown
  and reset flushes, and do not affect generation or ranking. Ollama's former
  `shownSuggestions` counter is reported more accurately as
  `returnedSuggestions`, with existing local counts migrated intact.
- Added a hidden, disabled-by-default Phase 2 project-retrieval source for
  deterministic inline completion. It uses monotonic effective-source versions
  scoped to a VFS lineage, a background-only module resolver supplied with the
  already-loaded project model, and collision-checked immutable exact-prefix
  tables under explicit source, query, file-count, and 16 MiB corpus caps.
  Edits only advance the VFS version and coalesce per-file rebuilds for 300 ms;
  workers build at most 16 files per batch, and the coordinator adopts results
  only when their lineage, eligibility generation, and source versions remain
  current. Base remerges and disposal of replaced corpora also stay off the
  coordinator's latency-sensitive path. Corpus lifecycle, memory, cancellation,
  and rejection counters are reported by `tua/cacheStats` and `tua/health`.
  The request-time source follows the tune-only conservative policy: it runs
  after semantic and current-file sources, only in comma, operator, and other
  contexts, after six non-whitespace prefix characters, and requires two
  supporting project matches before returning their common prefix. Same-folder
  and bundled-corpus evidence are excluded. Per-file callback declarations are
  indexed for later duplicate suppression, but callback generation remains
  deferred until it is evaluated. The server recognizes the internal
  `sources.project` switch for integration testing, but the VS Code extension
  does not contribute or document that setting. No Phase 2 held-out or external
  gate has run, so project retrieval is not release-approved.
- Experimental deterministic inline completion can now suggest one locally
  validated ghost-text item from Tua's existing statement snippets or repeated
  lines in the current file. It is opt-in through
  `tua.completion.inline.enabled`; semantic snippets and current-file recall
  have separate source switches. Requests are debounced, version-bound,
  and cancellable. After the 65 ms debounce, the extension gives the local
  backend 250 ms to respond; expiry cancels the linked request and abstains
  without falling through to Ollama. Candidate generation and validation use a
  10 ms core deadline, including a pre-parse check that abstains when the
  remaining budget is insufficient; queue, protocol, and end-to-end latency are
  measured separately. The deterministic query never builds an analysis
  snapshot, runs the checker, contacts a network service, or persists source
  patterns. With the publicly exposed Phase 1 sources, it reads only the active
  document; the hidden Phase 2 project experiment instead reads its immutable
  in-memory corpus and live VFS versions. The unified VS Code provider tries the
  deterministic backend first when it is
  enabled and uses the separately opted-in loopback Ollama backend only when
  that backend is disabled, returns no candidate, or is unsupported by an
  older server. Transient local cancellation, stale-document, and capacity
  failures abstain instead of starting model work. This first phase has no
  publicly enabled project corpus, cross-project history, n-gram generation,
  or multiline
  recall. Disabled and malformed requests do not enter the speculative queue;
  a panicking speculative request returns an internal error without killing
  the worker, and newest-wins cancellation suppresses superseded results.
- Developer-only menu replay capture can record the real VS Code
  `selectedCompletionInfo` state for Phase 1 evaluation. It is inactive unless
  both `TUA_INLINE_REPLAY_ROOT` and `TUA_INLINE_REPLAY_CAPTURE` are set, accepts
  records only while `tua.completion.inline.enabled` is on, accepts only files
  below that dataset root, and writes local JSONL containing
  dataset-relative paths, normalized source hashes, positions, document
  versions, edit delays, menu state, and the selected completion text and range
  when present. Selected text can retain source-derived names, so captures
  should be treated as potentially sensitive local evaluation data. CRLF
  documents hash the evaluator's canonical LF scenarios. The offline evaluator
  ingests it with `--menu-replay`, matches
  the edited-document hash and position to an exact evaluation scenario,
  rejects invalid records, reports unmatched edits, and splits Phase 1 results
  by real menu-open and menu-closed state. Nothing is uploaded.
- Added `make inline-editor-replay`, a macOS-only real-editor harness that uses
  VS Code's editor API for exact, per-character, hash-pinned `.tua` tuning
  scenarios and Chromium DevTools Protocol to observe the actual suggestion
  widget. It triggers Tua's inline provider and replays the local captures
  through the evaluator. Isolated Code
  profiles run under `sandbox-exec`; every extension host must prove loopback
  access and denied direct outbound access. A delayed mock-server case also
  proves that the provider sends a linked `$/cancelRequest` around its 250 ms
  timeout. Widget, capture, evaluator, cancellation, or isolation drift fails
  closed, and held-out/external data is never evaluated. Each successful replay
  also requires one or more expected deterministic source-counter increments
  around the captured request, with no return from a competing local source.
  This distinguishes a Tua result from identical text that
  VS Code may already show through suggestion preview. Evidence records native
  preview overlap, the Code build, host platform/architecture, repository
  revision, and artifact hashes; stale outputs are invalidated before startup
  and successful artifacts are atomically written with owner-only permissions.
  Tua now defaults `editor.suggest.preview` to `true`, so a selected menu item
  can be extended as ghost text. A unit test guards the product default, while
  the isolated replay workspace pins the same value for reproducible evidence.
  It does not force `.lua` files into Tua language mode because the
  production provider is not registered there.
  The reviewed clean-tree run matched all four curated tuning captures and
  cancelled delayed local work after 251 ms without rendering its late result.
- VS Code integration fixtures now preseed deterministic inline-completion
  settings before extension activation and wait for asynchronous editor-group
  switches, removing configuration and focus races on current Code builds.
- An offline evaluator for local inline completion (Phase 0 of the local
  inline-completion plan) measures candidate sources against real projects
  before any of them ships: precision, characters saved, wrong characters,
  latency, complete-parse cost, warm-up cache hit rates, and memory, by
  language, scenario, menu state, cursor context, and source.
  `make inline-eval` runs the tuning sets, `make inline-eval-smoke` gates CI on
  committed fixtures, and `make inline-eval-holdout` and
  `make inline-eval-external` check held-out and pinned third-party projects
  against committed, dataset-specific language thresholds and an exact pinned
  Phase 1 source configuration. The evaluator can require repeated support for
  full line-recall continuations while limiting one-off matches to a single
  token, allowing precision and coverage to be tuned independently.
- The representative LSP profile now emits schema-version-2 deterministic
  inline measurements: queue, worker-execution, and core-backend spans;
  scheduler-dispatch-to-response latency; client-observed request age
  after edits; cache and validation parse counters; and first diagnostics
  publication after an edit. Separate configurable budgets fail the profile on
  core/backend cost, server or end-to-end stalls, or diagnostics freshness
  without conflating those measurements with the offline evaluator's
  backend-only latency. The aggregate Phase 1 target runs representative
  fixtures with independent strict 10 ms limits for dispatch-to-response
  latency, worker execution, and core/backend work plus a 1,500 ms contended
  request-age ceiling before it checks every recorded phase prerequisite.
- A `VectorLength` standard lint rule (`TL4060`) warns when `#` is applied to a
  built-in `Vec2`. With LuaJIT's JIT off, as in LÖVE on Apple Silicon by
  default, vectors are plain tables and `#v` is `0`; the warning names the
  replacement, such as `v.length`, and the editor offers it as a quick fix.
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

- `TL1004` now explains the common case of a method called with `.` instead of
  `:`. When a dot call is short of arguments, the method's first parameter is
  `self`, and the receiver can be passed as that `self`, as with
  `ship.steer(1)` on an instance from another module, the message ends with the
  colon call to use, such as ``call it as `ship:steer(...)` ``. The class
  example in the language guide uses the colon form, which stays correct when
  a class moves to another module.
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
  repeating sheet-name lookups. Players created from animations of the bundled
  anim8 runtime, in generated and hand-written sheets alike, now share the
  source animation's read-only frame, duration, and interval arrays instead of
  copying them; each player still owns its timer, frame position, status, and
  flips. Code that edited a player's `frames`, `durations`, or `intervals` in
  place should replace the animation instead. Animations from replacement
  anim8 runtimes retain the public clone path. In the included macOS arm64
  LOVE 11.5.0 benchmark over 2,000 players, compared with the runtime before
  these sprite changes, construction memory fell from about 4.7 KB to 2.5 KB
  per player. With the JIT disabled, construction was about 6.3x faster, nested
  updates 1.16x, and leaf draws 1.6x; with the JIT enabled, they were about
  5.9x, 2.4x, and 2.3x faster. These figures are platform- and
  workload-specific; `make profile-sprite-love` compares another revision
  locally.
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

- The combined Phase 2 inline-completion evaluator now measures the actual host
  query against an adopted immutable project corpus, including production
  validation and stale-version rejection, instead of substituting a parallel
  evaluator-only retrieval path.
- The deterministic inline-completion evaluator now samples pauses after
  internal whitespace, so its enabled whitespace context contributes to
  quality results, and it no longer credits whitespace-only snippet literals
  as saved characters.
- `tua check`, builds, and the language server no longer overflow the stack on
  an unannotated Lua function that reassigns a parameter from its own method
  call, such as `str = str:gsub(...)`. Name inference resolved the receiver
  through the assignment it belonged to without bound, so any project with the
  bundled `anim8.lua` in its source tree crashed. An assignment's value is now
  evaluated as of just before the assignment, as in Lua, so a reassigned Lua
  value such as `n = n + 1` also keeps its type instead of becoming unknown.
  Multi-assignments evaluate every right-hand value against the bindings from
  before the whole statement, preserving Lua's swap semantics.
- Typed calls nested inside a dynamic call's arguments are now checked. For
  example, `print(add(1))` reports `TL1004` when `add` requires two arguments,
  just as a statement-level `add(1)` does.
- Typing an unclosed parameter list, such as `local function f(` above
  existing code, no longer collapses the following lines during parsing. The
  default-value eraser replaced their newlines with spaces, which shifted
  diagnostic positions in release builds and failed an internal assertion in
  debug builds.
- Hover now preserves project-inferred types for locals copied from required
  module fields. For example, `local flow = Globals.flow` now shows the same
  derived `StateMachine` type used by completion instead of the fast
  current-file path prematurely reporting `any`.
- Declared sprite sheets and players are now assignable to the built-in
  `SpriteSheet` and `SpritePlayer` types. Nested clips such as
  `children = { hat = { sheet = Hat } }`, `local sheet: SpriteSheet = Hat`, and
  functions taking a `SpriteSheet` or `SpritePlayer` previously reported
  `TL1003: expected string, found string`, because the generic types required
  every name parameter to accept any string. Declared sheets still check their
  exact animation, marker, and child names.
- Audio voice handles and analysis jobs now type-check their own methods.
  `voice:fadeTo(...)`, `voice:stop()`, `job:status()`, and the other handle
  methods were typed as if called on the audio system and reported
  `self.off: required field is missing`. `audio:subscribe` callbacks also
  receive the documented `previous` control state as a second parameter.
- `if value then` and `if a and b then` now narrow nilable values in the
  guarded branch, as `if value ~= nil then` already did. Calling a method on a
  nilable value inside such a branch previously reported
  `TL1003: expected Counter, found nil`.
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
