---
icon: lucide/terminal
---

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
- bundled helper runtimes such as `brinevector.lua`, `anim8.lua`, and
  `tuasignal.lua` are written only when generated Lua requires them;
- in-place builds are supported when `source` and `out` are the same directory;
- collisions between generated `.lua` and sibling/copied `.lua` files are
  reported as warnings.

When `emit_maps = true`, build also writes `*.lua.map.json` sidecars next to
generated Lua files. Their line mappings come from compiler lowering provenance,
including generated default guards, helper runtime imports, classes, runtime
checks, compound assignments, sprite expansions, static imports, and LuaCATS
annotation lines. Multiline erased types and non-ASCII source text retain their
original source-line ownership. Map files are line-based metadata only and are
not counted as emitted runtime files.

Use `--emit-on-error` when you want parseable/lowerable files to emit while the
command still exits non-zero:

```bash
cargo run -p tua-cli -- build --emit-on-error
```

## `traceback`

```bash
cargo run -p tua-cli -- traceback --maps build sentry-traceback.txt
cat sentry-traceback.txt | cargo run -p tua-cli -- traceback --maps build
```

Remaps generated Lua traceback locations back to Tua source using
`*.lua.map.json` sidecars from `tua build`. If `--maps` is omitted, the command
uses the configured output directory.

The regression suite builds Tua, executes the emitted Lua with a real local Lua
runtime, captures its stderr traceback, and verifies that `tua traceback`
returns the original Tua line. Linux CI installs Lua for this test; local runs
skip only that runtime case when no Lua executable is available.

For example, `build/main.lua:12` can become:

```text
src/main.tua:18 (generated build/main.lua:12)
```

Ambiguous basename-only frames are left unchanged instead of guessed.

## `watch`

```bash
cargo run -p tua-cli -- watch
```

Runs an initial build and rebuilds after debounced source changes. Watch mode
keeps one project analysis session alive, updates changed and removed semantic
files incrementally, and rechecks the transitive importer closure when an
exported module surface changes. Valid affected importers are re-emitted;
invalid importers are reported even when their source file did not change.

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
make fmt-check
make test
make verify
make ci-check
make build-features
make lsp-smoke
make lsp-features-smoke
```

`make verify` is non-mutating: it checks formatting, tests, strict Clippy,
extension activation, tracked JSON, and whitespace without rewriting source.
`make ci-check` adds bundled example builds, the documentation build, and the
benchmark smoke. Generated LOVE catalog verification still needs
`LOVE_API_ROOT`, while the real VS Code extension-host test remains
`make extension-test`.
