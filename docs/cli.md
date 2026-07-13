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

Loads project config, parses sources, runs type checking and enabled lints, and
prints diagnostics. The command exits non-zero when any diagnostic is reported.

## `fmt`

```bash
cargo run -p tua-cli -- fmt
cargo run -p tua-cli -- fmt --check
```

Formats project `.tua` source files in place using the built-in formatter.
`--check` reports files that would change and exits non-zero without writing.
Files with parse errors are left unchanged and reported with diagnostics.

## `lint`

```bash
cargo run -p tua-cli -- lint
cargo run -p tua-cli -- lint --strict
```

Runs the same project diagnostic pipeline as `check`, but reports success as
`linted N file(s)`. By default it respects `tua.toml`, including the default
formatting lints; `--strict` enables every optional lint while keeping
inline-ignore settings controlled by project/editor configuration.

## Diagnostic Timings

Set `TUA_TRACE=diagnostics` to print one stderr timing line for `check` and
`lint` diagnostic work:

```bash
TUA_TRACE=diagnostics cargo run -p tua-cli -- check
TUA_TRACE=diagnostics cargo run -p tua-cli -- lint
```

The trace reports `total`, `covered`, `unaccounted`, and fixed buckets for
`discover_files`, `read_vfs_update`, `parse`, `module_graph`,
`type_environment`, `checker`, `lints`, `inline_ignores`, and `publish`. Each
bucket is printed as milliseconds/count, for example `parse=2.413ms/21`.

## `build`

```bash
cargo run -p tua-cli -- build
```

Checks the project and writes emitted Lua to the configured output directory.
Emission is blocked by errors by default.

Build output mirrors the configured source tree:

- `.tua` files are checked, lowered, erased, and emitted as `.lua`;
- plain `.lua` files and other assets are copied unchanged;
- in-place builds are supported when `source` and `out` are the same directory;
- collisions between generated `.lua` and sibling/copied `.lua` files are
  reported as warnings.

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
cargo run -p tua-cli -- lsp --stdio
```

Starts the language server over stdio. Editors and tests call this command
directly. `--stdio` is accepted for editor-client compatibility and has the same
behavior as `tua lsp`.

## Make Targets

Common repository targets:

```bash
make test
make verify
make build-features
make lsp-smoke
make lsp-features-smoke
```
