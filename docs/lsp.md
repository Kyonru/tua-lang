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
information, copy analysis cache statistics, report an issue, and open Tua
settings. A language status item shows the server state, selected path, version,
and workspace directory when available.

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
- Optional ECS domain support can be enabled through `tua.ecs.enabled` or
  `[ecs].enabled`.
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
- Completion item resolve includes LOVE wiki links for generated LOVE API
  functions and object methods when a matching page is known.
- Asset path hovers show kind, relative path, inferred LOVE type, validation
  status, extension, and optional cheap metadata such as PNG dimensions when
  available.
- Asset path completion and validation use an in-memory asset index refreshed
  from workspace and watched-file events instead of walking asset directories
  during editor requests.
- Static LOVE asset diagnostics cover obvious missing files, unsupported
  formats, and source type mismatches before the game runs.
- Supported `value is Type` checks narrow true branches for hover, completion,
  and diagnostics; unsupported runtime targets report `TL1007`.
- Class constructor locals expose class fields and methods for completion,
  hover, diagnostics, and optimized final-method calls.
- LOVE object method completions prefer colon-call snippets and can rewrite
  `player.setF`-style completion sites to `player:setFilter(...)`.
- LOVE callback completions can insert declaration snippets for callbacks such
  as `love.load`, `love.update(dt)`, `love.draw`, and `love.conf(t)`.
- `love.conf(t)` parameters expose common config fields such as
  `t.window.title`, `t.window.width`, and `t.modules.physics`.
- Completion item resolve for documentation/details.
- Open buffers retain their last valid semantic snapshot. During a temporary
  syntax error, hover and inlay hints keep using those facts, while completion
  combines the current cursor context with the last valid type environment.
- Signature help triggered by `(` and `,`, retriggered by `)`, including LuaDoc
  overloads, generated LOVE overload variants, optional parameters, return
  labels, and parameter documentation where catalog or annotations provide it.
- Definition, references, rename, document highlight, document symbols,
  workspace symbols, document links, type definition, implementation, code
  actions, code lens references, call hierarchy, inlay hints, and full, range,
  and delta semantic tokens.
- Source actions for organizing imports, sorting imports, removing unused static
  import bindings, and fixing all safe diagnostics in the current file.
- Workspace file-rename edits for static `require(...)` and `import(...)`
  module specifiers when the move is unambiguous in the project model.
- Workspace folder support and watched-file registration for `.tua`, `.lua`,
  LOVE asset extensions, and `tua.toml`.

## Custom VS Code Feature Evaluation

The VS Code extension intentionally stays on stable VS Code APIs plus standard
LSP requests. The only custom document surface is the `tua:/builtins` virtual
document provider used for generated builtin and LOVE stubs.

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

- `TL4032`: a local variable changes clear concrete type.
- `TL4033`: a known table shape gains a late field or changes a field type.
- `TL4034`: a loop writes through a dynamic table key.

These diagnostics are disabled by default. `any` and unknown dynamic values
stay permissive, and the guidance never changes emitted Lua.

## ECS Domain Support

When enabled, the language server adds a tiny-ecs-shaped static surface:

- completion, hover, and signature help for `ecs.world`, `ecs.component`,
  `ecs.system`, and world methods such as `world:add` and `world:query`;
- `TL5002` diagnostics for clear component value mismatches;
- `TL5003` diagnostics for empty or unknown component query lists.

This support is editor/checker metadata. Runtime code still uses ordinary Lua
modules such as a tiny-ecs adapter required by the project.

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
make extension-package
```

For local smoke tests:

```bash
make lsp-smoke
make lsp-features-smoke
```

## Profiling

Profiling is disabled by default. Normal CLI and LSP output stays clean unless
the `TUA_PROFILE` environment variable is set.

To profile request latency against the checked-in small, medium, and large LSP
benchmark fixtures:

```bash
make profile-lsp
```

The command runs `benchmarks/lsp/small`, `benchmarks/lsp/medium`, and
`benchmarks/lsp/large`, reports average, p95, min, and max latency per LSP
request, and prints analysis cache hit/miss counters from the server. The
profile includes `completion`, `completionResolve`, `signatureHelp`,
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
of failing the extension host.

For editor-side trace logging, set `tua.trace.server` to `off`, `messages`, or
`verbose`, and set `tua.log.verbosity` to `off`, `error`, `info`, `debug`, or
`trace`. The extension passes those values in initialization options and sends
`tua/setTrace` on configuration changes so supporting servers update without a
restart.

Current lookup regression targets from the checked-in fixtures are intentionally
small: `definitionAgain`, `references`, `prepareRename`, and `rename` should
stay in the low single-digit millisecond range on the large fixture after
warmup. Completion, completion resolve, signature help, hover, and inlay hints
should reuse cached semantic facts instead of rebuilding type environments per
request. If a change pushes those rows noticeably higher, rerun with
`TUA_PROFILE=1` and inspect the `analysis.query.*`, `analysis.semantic.*`,
`analysis.completion.*`, `analysis.references.*`, and `lsp.*` spans before
merging.
