# Examples

The repository includes examples for language features and LOVE workflows.

## `examples/features`

A buildable tour of the language and LSP features.

```bash
make build-features
make lsp-features-smoke
```

Use this when changing language-server behavior.

The feature tour includes placeholder assets under `src/assets/` so static LOVE
asset validation, asset-path completion, and asset metadata hovers can run
without depending on real art or audio files.

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

## LSP Benchmark Fixtures

`benchmarks/lsp/small`, `benchmarks/lsp/medium`, and `benchmarks/lsp/large`
are checked-in Tua projects for profiling editor latency and cache behavior.
They include comments describing the feature surface each file stresses.

```bash
make profile-lsp
```

## Examples Speed Suite

The real bundled examples can also be profiled as a regression suite:

```bash
make profile-examples-smoke
make profile-examples
make profile-examples-baseline
```

The suite discovers every `examples/*/tua.toml`, times CLI commands and LSP
editor requests, and writes JSON reports under `target/tua-speed/`. It uses
local soft thresholds for comparisons because timing varies by machine:

```bash
TUA_SPEED_BASELINE=target/tua-speed/examples-baseline.json make profile-examples
```
