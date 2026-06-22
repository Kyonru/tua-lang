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
  LOVE items, modules, and inferred unannotated locals where possible.
- Completion for type aliases, local symbols, table fields, Lua builtins, LOVE
  items, require paths, and required-module fields.
- Completion item resolve for documentation/details already attached to core
  completion data.
- Signature help triggered by `(` and `,`.
- Definition, references, rename, document highlight, document symbols,
  workspace symbols, document links, inlay hints, and semantic tokens.
- Workspace folder support and watched-file registration for `.tua`, `.lua`,
  and `tua.toml`.

## Design Notes

Tua currently has broad LSP surface coverage. The next major improvement is
depth: a shared project analysis snapshot that every request can query instead
of re-reading and recomputing required modules independently.

The roadmap tracks this under the LuaHelper and rust-analyzer parity phases.

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
