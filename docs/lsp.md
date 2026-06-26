# Language Server

Tua ships a stdio language server through:

```bash
tua lsp
```

The VS Code extension starts this server and surfaces editor features for
`.tua` files.

## Current Capabilities

- Incremental document sync with open, change, save, and close notifications.
- Diagnostics on open, change, save, and watched-file updates.
- Optional formatting consistency diagnostics can be enabled through
  `tua.diagnostics.formattingLints` or `[diagnostics].formatting_lints`.
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
- LOVE object method completions prefer colon-call snippets and can rewrite
  `player.setF`-style completion sites to `player:setFilter(...)`.
- Completion item resolve for documentation/details.
- Signature help triggered by `(` and `,`, including parameter documentation
  from supported annotations.
- Definition, references, rename, document highlight, document symbols,
  workspace symbols, document links, code actions, call hierarchy, inlay hints,
  and semantic tokens.
- Workspace folder support and watched-file registration for `.tua`, `.lua`,
  and `tua.toml`.

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
