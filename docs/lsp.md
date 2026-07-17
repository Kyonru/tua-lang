---
icon: lucide/server
---

# Language Server

Tua ships a stdio language server through:

```bash
tua lsp
```

The VS Code extension starts this server and surfaces editor features for
`.tua` files.

The extension also contributes command-palette actions to restart the language
server, focus the Tua output channel, select a server binary, copy debug
information, copy analysis cache statistics, visualize SignalBus events,
inspect object-pool lifecycles, report an issue, and open Tua settings. A
language status item shows the server state, selected path, version, and
workspace directory when available.

## Current Capabilities

- Incremental document sync with open, change, save, and close notifications.
- Diagnostics on open, change, save, and watched-file updates. Clients without
  pull-diagnostic support receive `textDocument/publishDiagnostics`; while
  typing, `didChange` publishes a fast current-file lane first, then a full
  project-aware lane after a 200ms debounce. Newer document versions cancel
  stale queued slow diagnostics before they publish.
- Pull-diagnostic clients receive `diagnosticProvider`, can request
  `textDocument/diagnostic`, and receive `workspace/diagnostic/refresh` after
  configuration, watched-file, or workspace graph changes. Pull and push
  diagnostics use the same cached project snapshot for final results.
- Formatting consistency diagnostics are enabled by default and can be
  overridden through `tua.diagnostics.formattingLints` or
  `[diagnostics].formatting_lints`.
- Full-document, range, and on-type formatting are provided through
  `textDocument/formatting`, `textDocument/rangeFormatting`, and
  `textDocument/onTypeFormatting`. All three use `[format]` plus
  `tua.format.*` editor settings.
- Optional inline diagnostic suppressions can be enabled through
  `tua.diagnostics.inlineIgnores` or `[diagnostics].inline_ignores`.
- Optional LuaJIT shape guidance can be enabled through `tua.jit.enabled` or
  `[jit].enabled`.
- Built-in ECS support is enabled by default and can be disabled through
  `tua.ecs.enabled` or `[ecs].enabled`.
- Hover for aliases, locals, parameters, functions, table fields, Lua builtins,
  generated LOVE API items, modules, annotation docs, and inferred unannotated
  locals where possible. Generated LOVE API hovers include direct LOVE wiki
  links when a matching page is known.
- Completion for type aliases, local symbols, table fields, Lua builtins,
  generated LOVE API items, generated LOVE enum values, LuaDoc
  tags/types/snippets, require paths, import paths, module fields,
  asset paths in LOVE asset-loader and native constructor strings, and
  auto-import candidates.
- Generated LOVE API function completions can insert signature-driven call
  snippets, including enum choice placeholders such as `"fill" | "line"`.
- Completion items advertise label-details support and include concise labels
  such as field types or symbol categories when known.
- Native callback fields are classified as functions, and completion details
  preserve callback parameter names, typed varargs, and return types using
  `(parameters) -> returns` syntax.
- Completion item resolve includes LOVE wiki links for generated LOVE API
  functions and object methods when a matching page is known. Generated LOVE
  callback items also include parameter names, types, and parameter
  documentation from the pinned catalog.
- Asset path hovers show kind, relative path, inferred LOVE type, validation
  status, extension, and optional cheap metadata such as PNG dimensions when
  available.
- Asset path completion and validation use an in-memory asset index refreshed
  from workspace and watched-file events instead of walking asset directories
  during editor requests.
- Definition on a validated asset string opens the resolved image, audio, font,
  shader, or video file. References returns every indexed Tua or Lua asset
  loader literal that resolves to the same file.
- Static LOVE asset diagnostics cover obvious missing files, unsupported
  formats, source type mismatches, and path casing that differs from the
  filesystem before the game runs. Path casing reports warning `TL2008`.
- Supported `value is Type` checks narrow true branches for hover, completion,
  and diagnostics; unsupported runtime targets report `TL1007`.
- Class constructor locals expose class fields and methods for completion,
  hover, diagnostics, and optimized final-method calls.
- LOVE object method completions prefer colon-call snippets and can rewrite
  `player.setF`-style completion sites to `player:setFilter(...)`.
- LOVE callback completions are generated from the pinned LOVE catalog and can
  insert declaration snippets for all known callbacks, including
  `love.load`, `love.update(dt)`, `love.filedropped(file)`, touch/gamepad
  callbacks, `love.draw`, and `love.conf(t)`. Completion details include
  callback and parameter documentation from the same catalog.
- Callback declarations are checked against generated LOVE signatures.
  Callbacks may omit unused Lua parameters, while misspelled names and required
  extra or explicitly incompatible parameters report `TL5006`. Close typos
  offer a rename quick fix.
- `love.conf(t)` parameters expose common config fields such as
  `t.window.title`, `t.window.width`, and `t.modules.physics`. Known literal
  options are checked, including `"desktop" | "exclusive"` for
  `t.window.fullscreentype`.
- Completion item resolve for documentation/details.
- Open buffers retain their last valid semantic snapshot. During a temporary
  syntax error, hover and inlay hints keep using those facts, while completion
  combines the current cursor context with the last valid type environment.
- Signature help triggered by `(` and `,`, retriggered by `)`, including LuaDoc
  overloads, generated LOVE overload variants, optional parameters, return
  labels, typed varargs, native callback aliases, and parameter documentation
  where catalog or annotations provide it.
- Definition, references, rename, document highlight, document symbols,
  workspace symbols, document links, type definition, implementation, code
  actions, code lens references, call hierarchy, inlay hints, and full, range,
  and delta semantic tokens.
- Source actions for organizing imports, sorting imports, removing unused static
  import bindings, adding inferred local annotations, extracting selected
  expressions to locals or parameterized local functions, inlining stable local
  bindings across resolved reads, generating named table types from stable
  local table literals, creating missing core LOVE callbacks (`load`, `update`,
  and `draw`), wrapping repeated zero-argument constructors in an `objectPool`
  helper, and fixing all safe diagnostics in the current file.
- Workspace file-rename edits for static `require(...)` and `import(...)`
  module specifiers, plus helper/native LOVE asset literals when the move is
  unambiguous in the project model.
- Workspace folder support and watched-file registration for `.tua`, `.lua`,
  LOVE asset extensions, and `tua.toml`.
- `tua.toml` participates in the LSP client selector. The server provides
  schema-backed config key/section completion, enum/default value completion,
  hover documentation, and unknown-key quick fixes without routing config files
  through normal Tua source analysis.
- The custom `tua/signalGraph` request returns typed SignalBus event calls from
  the current analysis snapshot with stable bus-aware event identities, call
  roles, bus expressions, resolved payload schemas, conflict flags, inferred
  emit payloads, insertion targets, URIs, and source ranges. The VS Code
  `Tua: Show Signal Graph` command uses this data for event, bus, file,
  listener, and emitter filtering, conflict inspection, source navigation, and
  typed listener/declaration creation without merging independent same-named
  events. Hover links can open the graph from SignalBus event strings, but the
  graph data still comes from this request.
- The low-priority `tua/moduleGraph` request returns the compiler's revisioned
  project module graph: Tua/Lua files, static `require` and `import` edges,
  resolved, missing, invalid, or duplicate resolution states, structured
  cycles, reverse-dependency counts, inferred exported surfaces, and one
  deterministic shortest inclusion path from a root candidate to each
  reachable module. A root candidate is a module with no resolved importer;
  a pure cycle may therefore have no root path. The VS Code module dependency
  explorer opens every edge, export, and inclusion step at its compiler-owned
  source range.
- The low-priority `tua/ecsGraph` request returns cached project ECS facts:
  static component declarations and schemas, systems, standalone and
  system-owned queries, resolved component identities, conservative callback
  access (`filter`, `read`, `write`, `readWrite`, or `unknown`), and
  non-diagnostic performance insights. Module-qualified component references
  resolve through normal compiler definitions. The VS Code ECS Explorer opens
  every declaration and query use at its compiler-owned range.
- The low-priority `tua/objectPoolGraph` request returns cached object-pool
  lifecycle facts: pool declarations, item types inferred from `create`
  callbacks, visible `acquire`, `release`, `clear`, and `prewarm` calls, and
  conservative lifecycle insights. The VS Code Object Pool Explorer opens every
  pool and lifecycle operation at its compiler-owned range.
- SignalBus event strings use the same semantic index for completion, payload
  signature help, hover, definition, references, and rename. Typed listener and
  `declare` callback conflicts publish `TL5005`; untyped or unresolved buses
  remain permissive.
- The general `tua/inspect` request returns structured, revisioned document
  inspector items. Sprite sheet items include declaration ranges, resolved
  image asset URIs and validation status, frame metadata, animations, and the
  same `TL5004` diagnostics used by normal project analysis. The VS Code sprite
  hover action reuses this inspector data.
- The low-priority `tua/assetInventory` request returns revisioned project
  assets grouped by stable category metadata, static Tua/Lua reference ranges,
  dimensions when cheaply available, and conservative `TL2009` hints for
  assets with no static references. VS Code asset hover previews reuse this
  inventory instead of resolving paths in the extension. Dynamic asset loading
  remains valid, so `TL2009` is a hint rather than proof that a file can be
  deleted.

## Syntax And Semantic Highlighting

The VS Code extension uses two complementary highlighting layers:

- The TextMate grammar runs immediately while typing and does not wait for the
  language server. It covers Lua syntax plus Tua types, classes, enums,
  function annotations, LuaDoc, modules, assets, SignalBus event names, LOVE,
  ECS, sprites, object pools, calls, members, labels, literals, and operators.
- Semantic tokens refine that baseline from compiler facts. Tua emits standard
  VS Code token types for classes, enums, types, functions, methods,
  parameters, variables, properties, namespaces, events, labels, and
  annotations, with declaration, readonly, deprecated, modification, and
  default-library modifiers.

Full-document, range, and delta requests use the same classification. During
temporarily invalid syntax, the semantic provider tokenizes the current source
lossily so declarations, members, types, built-ins, events, modules, and labels
do not all revert to plain text. VS Code themes can use their normal semantic
rules; the extension also supplies Tua-specific TextMate fallback scopes.

Use `Developer: Inspect Editor Tokens and Scopes` in VS Code to inspect both
the TextMate scope and semantic token at the cursor.

Member access is classified segment by segment. In `object.position.x`,
`object` keeps its variable or namespace classification while `position` and
`x` are properties. A terminal called member, such as `object.position:update()`,
is a method. The active VS Code theme chooses the actual colors. Themes that
render variables and properties identically can be overridden without changing
other Tua tokens:

```json
"editor.semanticTokenColorCustomizations": {
  "rules": {
    "property:tua": "#4EC9B0"
  }
}
```

## Custom VS Code Feature Evaluation

The VS Code extension intentionally stays on stable VS Code APIs plus standard
LSP requests. The `tua:/builtins` virtual document provider is used for
generated builtin and LOVE stubs. Experimental visual tooling, such as the
sprite animation preview command, is explicit and gated by `tua.preview.enabled`;
sprite sheet hovers may offer a trusted command link, but the preview itself is
still a command/webview rather than an LSP hover payload. Both the hover link
and preview request sprite facts through `tua/inspect`; the extension does not
parse Tua source or resolve asset paths independently.

Signal graph visualization is an explicit project-inspection command backed by
`tua/signalGraph`. The extension renders semantic results, filters by event,
bus, file, listener, and emitter, shows conflicting payload variants directly,
and can insert a typed listener or `declare(...)` schema from server-provided
call metadata. It does not maintain a parallel SignalBus parser.

Module graph visualization follows the same boundary. `Tua: Show Module
Dependency Graph` requests `tua/moduleGraph` and presents the returned semantic
snapshot without scanning source in JavaScript. `Tua: Why Is This Module
Included?` focuses the active file and opens its root-to-module explanation.
Module graph work uses the background request lane, so pending inspection
yields to document changes, completion, hover, definition, and other
interactive editor work.

ECS visualization follows the same compiler/presentation boundary. `Tua: Show
ECS Explorer` requests `tua/ecsGraph`; the extension filters and presents the
structured snapshot but does not parse ECS calls. The request uses the
background lane and yields to interactive editor work. Allocation findings are
returned as inspector insights only and are never published as diagnostics.

Object-pool visualization follows that same boundary. `Tua: Show Object Pool
Explorer` requests `tua/objectPoolGraph`; the extension presents pool
declarations, lifecycle operations, and conservative unreleased-acquire
warnings without parsing source in JavaScript. The warning is intentionally
narrow: it only reports a statically visible same-file local acquired from a
known pool when no later same-file `release(local)` or `clear()` call is found.

The Tua Assets Explorer is backed by `tua/assetInventory`. The extension only
groups and presents compiler results, publishes the returned unused-asset
hints, and starts normal VS Code file edits. Asset parsing, casing validation,
reference identity, and rename edits stay in shared compiler analysis.

TypeScript Go-style proposed editor features were evaluated and are not exposed
today:

- Verbose hover is not a separate protocol. Standard hover already carries type
  labels, docs, declaration context, module context, and LOVE wiki links.
- Source definition is redundant with `textDocument/definition`,
  `textDocument/typeDefinition`, and `tua:/builtins` virtual documents.
- Multi-document highlight is not automatic. Use references for project-wide
  occurrences; any future preview command must reuse the same references/index
  data rather than introduce a parallel query.
- Auto insert is out of scope until Tua has a precise Lua/Tua syntax trigger.
  Block helpers should use snippets or standard completion first.

## LOVE Method Guidance

The language server warns when a known LOVE object method is called with `.`
instead of `:`:

```tua
local player = image "assets/player.png"
player.setFilter("nearest")
```

The diagnostic is `TL4018` and suggests the colon-call form:

```tua
player:setFilter("nearest")
```

This mirrors Lua/LOVE runtime behavior: `:` passes the receiver as `self`, while
`.` does not.

The code-action list can apply that fix mechanically. It can also rewrite
obvious LOVE constructor calls to Tua helpers, for example
`love.graphics.newImage("assets/player.png")` to `image "assets/player.png"`,
or `love.audio.newSource("assets/theme.ogg", "stream")` to
`music "assets/theme.ogg"`.

## Inline Suppressions

When enabled, the LSP applies the same line-targeted suppression comments as
`tua check`:

```tua
-- tua-ignore-next-line TL4018
player.setFilter("nearest")

local legacy: number = "soon" -- tua-ignore-line TL1003
```

Invalid diagnostic codes and suppressions that do not match any diagnostic still
publish warnings. Suppression comments do not change hover, completion, inferred
types, or emitted Lua.

## Formatting

The language server advertises full-document, range, and on-type formatting for
Tua files. Full-document formatting returns one full-document edit when the file
would change, an empty edit list when the buffer is already formatted, and no
edit for syntax-invalid buffers. Range formatting returns local edits that stay
inside the requested range. On-type formatting uses the same formatter but only
returns stable local edits for Lua/Tua-relevant triggers such as newline,
braces/brackets/parens, comma, and `end`.

Formatter defaults come from `[format]` in `tua.toml`:

```toml
[format]
indent_width = 2
line_width = 100
quote_style = "preserve"
final_newline = true
```

VS Code can override those values with `tua.format.indentWidth`,
`tua.format.lineWidth`, `tua.format.quoteStyle`, and
`tua.format.finalNewline`.

## LuaJIT Guidance

When enabled, the LSP publishes conservative LuaJIT-friendly guidance from the
same diagnostic pipeline as `tua check`:

- `TL4032`: additional assignable representation drift when detectable.
- `TL4033`: a known table shape gains a late field or changes a field type.
- `TL4034`: a loop writes through a dynamic table key.

Incompatible inferred-local reassignment in `.tua` is a core `TL1003` error and
does not depend on JIT guidance. JIT diagnostics are disabled by default.
`any` and unknown dynamic values stay permissive, and guidance never changes
emitted Lua.

## ECS Domain Support

The language server provides the built-in ECS static surface by default:

- completion, hover, and signature help for `ecs.world`, `ecs.component`,
  `ecs.system`, and world methods such as `world:add` and `world:query`;
- `TL5002` diagnostics for clear component value mismatches;
- `TL5003` diagnostics for empty or unknown component query lists.

Builds inject `require("tuaecs")` and copy the bundled runtime only when the
built-in `ecs` module is used. An explicit local `ecs` binding remains an
ordinary Lua runtime override.

## Object Pool Support

The language server treats `objectPool({...})` as a built-in game-development
primitive:

- completion, hover, signature help, inlay hints, and builtin navigation for
  pool construction and lifecycle methods;
- factory-return inference, so an annotated `create(): Bullet` makes
  `pool:acquire()` return `Bullet`;
- typed `release`, `prewarm`, count, ownership, and cleanup methods;
- permissive `any` item behavior when the factory return is not annotated;
- conservative `TL5007` warnings for visible same-file acquires that do not
  reach a later same-file release or pool clear.

Builds inject `require("tuaobjectpool")` and copy the bundled runtime only when
`objectPool` is used.

## Design Notes

Tua has broad LSP surface coverage, but it stays conservative. Unknown symbols,
dynamic table shapes, calls through `any`, and unresolved module patterns should
usually produce no speculative editor result instead of a guessed one.
Type-definition and implementation requests follow the same rule: known aliases,
table shapes, module exports, and typed receiver methods navigate to concrete
origins, while ambiguous dynamic dispatch returns no locations.

The current gap breakdown lives in the [LSP parity matrix](lsp-parity.md).

## VS Code

The extension lives in `editors/vscode`.

Useful development commands:

```bash
make extension-deps
make extension-check
make extension-test
make extension-package
```

Tua relies on VS Code's language-scoped, zero-delay quick suggestions. The
extension does not observe every text edit or invoke
`editor.action.triggerSuggest`; this avoids duplicate completion requests and
stale timer callbacks. `make extension-test` types through a real editor host
and verifies request version, cursor position, freshness, and suggestion-widget
acceptance.

`Tua: Select Inlay Hint Mode`, available from the command palette and the Tua
editor toolbar, offers Full, Compact, Types Only, Parameters Only, and Off.
Compact uses VS Code's native inlay-hint maximum length and restores the exact
prior Tua language override when another mode is selected, including an absent
override. The remaining modes filter complete server-provided type and
parameter hints in the extension. Mode changes do not rewrite
`editor.inlayHints.enabled`, other inlay-hint preferences, or document analysis.

`Tua: Explain Type / Why Is This Any?` sends the cursor and synchronized
document revision through `tua/explainType`. The compiler returns an ordered,
source-linked explanation covering explicit annotations, initializer inference,
literal widening, control-flow narrowing, module resolution, member lookup,
and permissive fallbacks. The extension presents those steps in a quick pick;
selecting a located step opens its source. Dynamic causes distinguish explicit
`any`, unresolved symbols or modules, unknown call results, missing fields, and
`any` propagation instead of reporting one generic unknown result.

Type explanations use the same valid or degraded semantic snapshot as hover,
completion, and navigation. They currently target identifiers and dotted member
chains; arbitrary runtime expressions remain intentionally unexplained when no
stable compiler identity exists.

`Tua: Open Generated Lua Beside Tua` resolves the active saved Tua source
through the nearest `tua.toml` `source` and `out` settings and opens the emitted
`.lua` file in a side editor. The command is an editor convenience for inspecting
build output; it does not synthesize Lua or run `tua build` automatically.

`Tua: Jump Between Tua and Generated Lua` additionally reads the emitted
`*.lua.map.json` sidecar and jumps between the nearest mapped Tua and generated
Lua lines. Source maps currently carry line provenance only, so the selected
target column is 0.

`Tua: Copy Fully Qualified Symbol Name` and `Tua: Workspace Symbols by Module`
reuse the standard document-symbol and workspace-symbol providers. Qualified
names use the compiler module path from `tua.toml` source roots plus the
selected symbol path, and the workspace quick pick groups symbols under their
module container names.

For local smoke tests:

```bash
make lsp-smoke
make lsp-features-smoke
```

The required VS Code extension-host suite runs with:

```bash
npm --prefix editors/vscode run test:vscode
```

It opens an isolated two-root workspace and verifies live completion freshness,
cancellation, hover, definition navigation, signature help, inlay hints,
UTF-16 and untitled-document positions, module and asset file-rename edits,
module dependency graph, ECS Explorer, Object Pool Explorer, Asset Explorer,
and sprite preview opening/reload/focus behavior. Sprite tests also assert that
inspection uses the current synchronized document version. CI runs the same
suite under Xvfb.

## Profiling

Profiling is disabled by default. Normal CLI and LSP output stays clean unless
the `TUA_PROFILE` environment variable is set.

To profile request latency against the checked-in small, medium, and large LSP
benchmark fixtures:

```bash
make profile-lsp
```

The command runs `benchmarks/lsp/small`, `benchmarks/lsp/medium`, and
`benchmarks/lsp/large`, reports average, p50, p95, p99, min, and max latency
per LSP request, and prints analysis cache hit/miss counters from the server.
The profile includes `completion`, `completionResolve`, `signatureHelp`,
`definition`, `definitionAgain`, `references`, `prepareRename`, and `rename`
so autocomplete, inference, navigation, and rename regressions are visible in
one run. `definitionAgain` repeats the same go-to-definition request
immediately so warm-snapshot regressions are easy to spot.

Useful knobs:

```bash
TUA_PROFILE_REPEAT=20 make profile-lsp
TUA_PROFILE=1 make profile-lsp
TUA_PROFILE=1 TUA_PROFILE_STREAM=1 make profile-lsp
```

For completion-specific profiling and ranking regression checks:

```bash
make profile-completion
make completion-ranking-smoke
make profile-representative
```

`make profile-completion` runs the fixed `benchmarks/lsp/medium` profile and
prints completion p50/p95/p99, completion-context cache hit rate,
`analysis.completion.ranking` overhead, and optional model-lane fallback
frequency. See [Completion Benchmarks](completion-benchmarks.md) for latency
targets, baseline comparison, ranking diagnostics, and known failure modes.

`make profile-representative` generates exact 1,000-line, 5,000-line,
100-file, and 1,000-file fixtures under `target/`, then measures cold and warm
completion, hover, definition, and inlay hints. Its contention phase sends a
background request burst before `didChange` and completion, so cancellation,
request age, freshness, and stale-result regressions are observable rather
than inferred from serial timings.

Experimental contextual sequence ranking can be enabled with
`tua.completion.sequenceRanking.enabled`. The initial provider is local-only,
bounded by `tua.completion.sequenceRanking.timeoutMs`, and receives only a
normalized context snapshot plus compiler candidates. Timeout or provider error
returns zero score hints and keeps the pre-sequence completion ordering.

Experimental multi-token completion can be enabled with
`tua.completion.multiToken.enabled`. It runs as a separate lane after compiler
completion, validates each insertion by parsing the edited source, and appends
only valid suggestions. `tua.completion.multiToken.allowModelIntegration` is a
future policy gate; the current provider is local-only and performs no network
or model calls.

To profile the real bundled examples across both CLI commands and editor
requests:

```bash
make profile-examples
make profile-examples-smoke
make profile-examples-baseline
```

`make profile-examples` discovers every `examples/*/tua.toml`, times `check`,
`lint`, `fmt --check`, `build`, and LSP request latency, then writes
`target/tua-speed/examples-profile.json`. The build timing runs in a temporary
copy so generated output does not dirty the example projects. Set
`TUA_SPEED_BASELINE=target/tua-speed/examples-baseline.json` to compare against
a previous run; timing regressions are warnings by default and can be made
fatal with `TUA_SPEED_FAIL_ON_REGRESSION=1`.

For diagnostic-specific timing, start the server with `TUA_TRACE=diagnostics`.
The trace writes stderr lines after `textDocument/publishDiagnostics` without
touching LSP stdout:

```bash
TUA_TRACE=diagnostics cargo run -p tua-cli -- lsp --stdio
```

Each line reports `total`, `covered`, `unaccounted`, `discover_files`,
`read_vfs_update`, `parse`, `module_graph`, `type_environment`, `checker`,
`lints`, `inline_ignores`, and `publish`. Buckets are milliseconds/count pairs
so slow diagnostic phases are easy to compare across edits.

`TUA_PROFILE=1` enables per-phase tracing from the server. By default the
profiling script prints those traces after each fixture; `TUA_PROFILE_STREAM=1`
streams them live to stderr. The same tracing can be enabled for any manual LSP
session with:

```bash
TUA_PROFILE=1 tua lsp
```

The VS Code extension also exposes runtime-facing profiling commands:
`Tua: Save Cache Stats` writes the current `tua/cacheStats` response as JSON
under the extension's global `profiles` directory outside the workspace by
default, while `Tua: Save CPU Profile` and `Tua: Save Heap Summary` ask the
server for Rust-relevant profile artifacts in that same output directory.
Servers without those profile artifact capabilities return a warning instead
of failing the extension host. Cache stats include `usageIndex` hits and misses
for the compact per-file member/event summaries used by project completion and
the signal graph; unchanged files should contribute hits across snapshot
revisions. They also include a `workspaceWarm` object with discovered and
warmed and failed file counts, high/low queue depth, stale-result count, and the
revision of the prepared project snapshot. Warm coverage is reported separately
from foreground cache hits so idle indexing cannot make interactive hit rates
look better than they are.

Workspace warming runs only while the request scheduler is idle. It prioritizes
open and changed files, then direct dependency neighbors, then the remaining
`.tua` and `.lua` files. Completion, hover, definition, signature help, and
viewport inlay requests continue on the interactive worker lane while warming
is active. A document edit invalidates stale warm results; the coordinator only
adopts artifacts produced for matching analysis inputs.

For editor-side trace logging, set `tua.trace.server` to `off`, `messages`, or
`verbose`, and set `tua.log.verbosity` to `off`, `error`, `info`, `debug`, or
`trace`. The extension passes those values in initialization options and sends
`tua/setTrace` on configuration changes so supporting servers update without a
restart.

Current lookup regression targets from the checked-in fixtures are intentionally
small: `definitionAgain`, `references`, `prepareRename`, and `rename` should
stay in the low single-digit millisecond range on the large fixture after
warmup. Completion p95 on the fixed medium profile should stay at or below 50ms,
and ranking p95 should stay at or below 5ms. Completion, completion resolve,
signature help, hover, and inlay hints should reuse cached semantic facts
instead of rebuilding type environments per request. If a change pushes those
rows noticeably higher, rerun with `TUA_PROFILE=1` and inspect the
`analysis.query.*`, `analysis.semantic.*`, `analysis.completion.*`,
`analysis.references.*`, and `lsp.*` spans before merging.
