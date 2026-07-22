---
icon: lucide/flask-conical
---

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

It also includes focused modules for the Baton-backed `input({...})` helper,
state machines, stores, Signals, ECS, object pools, sprite sheets, vectors, and
Lua interoperability.

## `examples/linter`

An intentionally failing diagnostic fixture with one named example for every
currently emitted standard lint rule.

```bash
cargo build -p tua-cli
cd examples/linter
../../target/debug/tua lint
```

The lint command exits with status `1` by design. The fixture has no
`tua.toml`, so it does not participate in buildable-example or benchmark runs.

## `examples/love-minimal`

A small LOVE project for basic compile-and-run validation.

```bash
make build-love-minimal
make run-love-minimal
```

!!! info "LOVE is required only for run targets"
    Build targets work without LOVE. `make run-love-minimal` and
    `make run-love-full` require a local LOVE installation.

## `examples/love-full`

A larger LOVE conversion example with Lua interop.

```bash
make build-love-full
make run-love-full
```

The run target uses a smoke mode suitable for local validation.

## `examples/love-tua`

A small playable bullet hell that combines Tua's ECS, object pool, store, state
machine, and input helpers with a hand-written Lua projectile-pattern module
and declaration-typed [feel.lua](https://github.com/Kyonru/feel.lua) feedback.

```bash
make check-love-tua
make run-love-tua
```

The `.tua` and `.lua` sources live together at the example root, including the
tracked Feel runtime and declarations under `lib/feel`. Tua indexes the
declarations without emitting them, copies runtime Lua unchanged, and writes
the runnable project under the ignored `build/` directory. Because the example
is self-contained, it participates in the default example build and benchmark
suites without network setup.

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
Larger deterministic fixtures are generated under `target/` so 1,000 generated
project files do not live in the repository.

```bash
make profile-lsp
make profile-completion
make completion-ranking-smoke
make generate-lsp-benchmarks
make profile-representative
```

## Examples Speed Suite

The real bundled examples can also be profiled as a regression suite:

```bash
make benchmark-smoke
make profile-examples-smoke
make profile-examples
make profile-examples-baseline
make profile-all
```

The examples suite discovers every `examples/*/tua.toml` and times CLI commands
and LSP editor requests. The representative suite measures generated large-file
and project-scale fixtures, including request contention. Both write JSON
reports under `target/tua-speed/`. Example timing comparisons use local soft
thresholds because timing varies by machine:

```bash
TUA_SPEED_BASELINE=target/tua-speed/examples-baseline.json make profile-examples
```
