# Language Server

Tua ships a stdio language server through:

```bash
tua lsp
```

The VS Code extension starts this server and surfaces editor features for
`.tua` files.

## Current Capabilities

- Incremental document sync with open, change, save, and close notifications.
- Diagnostics on open, change, save, and watched-file updates. While typing,
  `didChange` publishes a fast current-file lane first, then a full
  project-aware lane after a 200ms debounce. Newer document versions cancel
  stale queued slow diagnostics before they publish.
- Optional formatting consistency diagnostics can be enabled through
  `tua.diagnostics.formattingLints` or `[diagnostics].formatting_lints`.
- Full-document formatting is provided through `textDocument/formatting` and
  uses `[format]` plus `tua.format.*` editor settings.
- Optional inline diagnostic suppressions can be enabled through
  `tua.diagnostics.inlineIgnores` or `[diagnostics].inline_ignores`.
- Optional LuaJIT shape guidance can be enabled through `tua.jit.enabled` or
  `[jit].enabled`.
- Optional ECS domain support can be enabled through `tua.ecs.enabled` or
  `[ecs].enabled`.
- Hover for aliases, locals, parameters, functions, table fields, Lua builtins,
  generated LOVE API items, modules, annotation docs, and inferred unannotated
  locals where possible.
- Completion for type aliases, local symbols, table fields, Lua builtins,
  generated LOVE API items, LuaDoc tags/types/snippets, require paths,
  required-module fields, asset paths in LOVE asset-loader strings, and
  auto-import candidates.
- Asset path hovers show kind, relative path, inferred LOVE type, validation
  status, extension, and optional cheap metadata such as PNG dimensions when
  available.
- Asset path completion and validation use an in-memory asset index refreshed
  from workspace and watched-file events instead of walking asset directories
  during editor requests.
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
- Signature help triggered by `(` and `,`, including parameter documentation
  from supported annotations.
- Definition, references, rename, document highlight, document symbols,
  workspace symbols, document links, code actions, call hierarchy, inlay hints,
  and semantic tokens.
- Workspace folder support and watched-file registration for `.tua`, `.lua`,
  LOVE asset extensions, and `tua.toml`.

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

The language server advertises `documentFormattingProvider: true` for Tua files.
Formatting returns one full-document edit when the file would change, an empty
edit list when the buffer is already formatted, and no edit for syntax-invalid
buffers.

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

Current lookup regression targets from the checked-in fixtures are intentionally
small: `definitionAgain`, `references`, `prepareRename`, and `rename` should
stay in the low single-digit millisecond range on the large fixture after
warmup. Completion, completion resolve, signature help, hover, and inlay hints
should reuse cached semantic facts instead of rebuilding type environments per
request. If a change pushes those rows noticeably higher, rerun with
`TUA_PROFILE=1` and inspect the `analysis.query.*`, `analysis.semantic.*`,
`analysis.completion.*`, `analysis.references.*`, and `lsp.*` spans before
merging.
