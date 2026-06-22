# Examples

The repository includes examples for language features and LOVE workflows.

## `examples/features`

A buildable tour of the language and LSP features.

```bash
make build-features
make lsp-features-smoke
```

Use this when changing language-server behavior.

## `examples/love-minimal`

A small LOVE project for basic compile-and-run validation.

```bash
make build-love-minimal
make run-love-minimal
```

`make run-love-minimal` requires LOVE to be installed.

## `examples/love-full`

A larger LOVE conversion example with Lua interop.

```bash
make build-love-full
make run-love-full
```

The run target uses a smoke mode suitable for local validation.

## Syntax Validation

When LuaJIT or Lua is available:

```bash
make lua-syntax
```

The test skips gracefully when no compatible Lua executable is present.
