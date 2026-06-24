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
- Hover for aliases, locals, parameters, functions, table fields, Lua builtins,
  generated LOVE API items, modules, annotation docs, and inferred unannotated
  locals where possible.
- Completion for type aliases, local symbols, table fields, Lua builtins,
  generated LOVE API items, LuaDoc tags/types/snippets, require paths,
  required-module fields, and auto-import candidates.
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
