# CLI

The Tua CLI is provided by the `tua-cli` crate.

During development, run commands through Cargo:

```bash
cargo run -p tua-cli -- <command>
```

## `init`

```bash
cargo run -p tua-cli -- init
```

Creates `tua.toml` and `src/` if they do not already exist.

## `check`

```bash
cargo run -p tua-cli -- check
```

Loads project config, parses sources, runs type checking, and prints diagnostics.

## `build`

```bash
cargo run -p tua-cli -- build
```

Checks the project and writes emitted Lua to the configured output directory.
Emission is blocked by errors by default.

Use `--emit-on-error` when you want parseable/lowerable files to emit while the
command still exits non-zero:

```bash
cargo run -p tua-cli -- build --emit-on-error
```

## `watch`

```bash
cargo run -p tua-cli -- watch
```

Runs an initial build and rebuilds after debounced source changes.

## `lsp`

```bash
cargo run -p tua-cli -- lsp
```

Starts the language server over stdio. Editors and tests call this command
directly.

## Make Targets

Common repository targets:

```bash
make test
make verify
make build-features
make lsp-smoke
make lsp-features-smoke
```
