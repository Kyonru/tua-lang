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

Creates `tua.toml`, `src/`, and `src/main.tua` if they do not already exist.

## `check`

```bash
cargo run -p tua-cli -- check
```

Loads project config, parses sources, runs type checking and configured project
diagnostics, and prints them. Standard lint rules are enabled by default; set
`diagnostics.rules = false` for a type/project-only check. The command exits
non-zero for errors and still prints non-blocking warnings, information, and
hints. Use `tua check --deny-warnings` when CI should reject warnings too.

## `fmt`

```bash
cargo run -p tua-cli -- fmt
cargo run -p tua-cli -- fmt --check
```

Formats project `.tua` source files in place using the built-in formatter.
`--check` reports files that would change and exits non-zero without writing.
Files with parse errors are left unchanged and reported with diagnostics.
Formatting does not produce lint diagnostics; see [Formatter](formatter.md).

## `lint`

```bash
cargo run -p tua-cli -- lint
cargo run -p tua-cli -- lint --strict
```

Runs syntax diagnostics and the standard lint rules, reporting success as
`linted N file(s)`. Type, project, and domain diagnostics remain the
responsibility of `tua check`. `--strict` additionally enables every broadly
applicable Tua-specific optional lint while keeping the noisy
direct-call-only `uncalled_local_functions` heuristic opt-in and inline-ignore
settings controlled by project/editor configuration. Lint exits non-zero for warning or error
diagnostics; information and hints remain advisory. See [Linter](linter.md) for
rule compatibility.

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

Configured [asset pipelines](asset-pipelines.md) run or restore their cached
outputs before source discovery, so generated `.tua` and `.d.tua` modules use
the same checking and emission path as handwritten source.

Build output mirrors the configured source tree. The configured `main` source
file also emits to `<out>/main.lua` for LOVE entrypoint loading while keeping
its normal mirrored output path.

- `.tua` files are checked, lowered, erased, and emitted as `.lua`;
- plain `.lua` files and other assets are copied unchanged;
- bundled helper runtimes such as `brinevector.lua`, `anim8.lua`,
  `tuasignal.lua`, `tuaecs.lua`, `tuaobjectpool.lua`, `tuastatemachine.lua`,
  `tuastore.lua`, `tuaaudio.lua`, `baton.lua`, `tuatween.lua`, and
  `tuacamera.lua` are written only when
  generated Lua requires them. The audio runtime also brings in its
  `tuasignal.lua` and `tuastore.lua` dependencies;
- in-place builds are supported when `source` and `out` are the same directory;
- collisions between generated `.lua` and sibling/copied `.lua` files are
  reported as warnings.

When `emit_maps = true`, build writes two source-map sidecars next to each
generated Lua file:

- `*.lua.map.json` is Tua's explicit provenance format used by generated-Lua
  navigation and `tua traceback`;
- `*.lua.map` is a standard source map v3 file used by compatible Lua debuggers.

Both derive from compiler lowering provenance, including generated default
guards, helper runtime imports, classes, runtime checks, compound assignments,
sprite expansions, static imports, and LuaCATS annotation lines. Multiline
erased types and non-ASCII source text retain their original source-line
ownership. Map files are line-based metadata only and are not counted as
emitted runtime files.

!!! warning "Recovery emission still reports failure"
    Use `--emit-on-error` when you want parseable/lowerable files to emit while
    errors remain. The command still exits non-zero. Files with syntax or
    lowering errors are skipped. `TL0004` rejects parser-recognized syntax that
    is not part of Tua; `TL2010` is the final safety failure when completed
    compiler output is not valid Lua 5.1.

    ```bash
    cargo run -p tua-cli -- build --emit-on-error
    ```

## `package`

```bash
cargo run -p tua-cli -- package
cargo run -p tua-cli -- package --target windows --love path/to/love-11.5-win64
cargo run -p tua-cli -- package --target macos --love /Applications/love.app
```

Builds the project, then writes `dist/<name>.love`. `--target windows` or
`--target macos` also fuses a standalone game with the LOVE runtime passed to
`--love`; packaging never downloads anything. `--name` sets the game name
(default: the project directory's name) and `--identifier` the macOS bundle
identifier. See [Packaging Games](packaging.md).

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

Runs an initial build, registers the platform's native filesystem watcher, and
rebuilds after a quiet debounce window. If native notifications are unavailable
or fail, watch mode switches to a content-comparing polling backend.

Filesystem events are treated as wake-up hints. One content-aware snapshot
produces the authoritative changed/deleted batch, so create, remove, rename,
Tua/Lua source, and copied asset changes share the same path. Files mentioned by
an event are rehashed even when size and modification time are unchanged.
Native mode also performs a low-frequency reconciliation scan to recover from
platform events that are silently dropped.

Watch mode keeps one project analysis session alive, updates changed and removed
semantic files incrementally, and rechecks the transitive importer closure when
an exported module surface changes. Valid affected importers are re-emitted;
invalid importers are reported even when their source file did not change.
Changing, creating, or removing `tua.toml` reloads the project layout and starts
a fresh warm analysis session for the new source root, output root, extension,
and diagnostics settings. Invalid configuration is reported while the previous
valid configuration remains active.

Input roots declared by [asset pipelines](asset-pipelines.md) are watched even
when they live outside `source`. A changed input reruns its content-addressed
importer before Tua rebuilds the generated module consumers.

## `dev`

```bash
cargo run -p tua-cli -- dev
cargo run -p tua-cli -- dev --love /path/to/love
```

Builds like `watch`, starts LOVE on the output, and hot reloads the running
game after every error-free rebuild. `--love` or `TUA_LOVE` chooses the LOVE
executable; otherwise `love` on `PATH` is used, then `/Applications/love.app`
on macOS. Closing the game ends the command. See [Hot Reload](hot-reload.md).

## `lsp`

```bash
cargo run -p tua-cli -- lsp
cargo run -p tua-cli -- lsp --stdio
```

Starts the language server over stdio. Editors and tests call this command
directly. `--stdio` is accepted for editor-client compatibility and has the same
behavior as `tua lsp`.

## `api`

```bash
cargo run -p tua-cli -- api --stdio
cargo run -p tua-cli -- api --stdio --workspace /path/to/project
```

Starts the read-only, newline-delimited JSON-RPC API. It keeps one project
analysis session alive and refreshes disk state only after
`tua.api/refresh`. See [External API And MCP](external-api-bridge.md).

## `mcp`

```bash
cargo run -p tua-cli -- mcp --stdio
cargo run -p tua-cli -- mcp --stdio --workspace /path/to/project
```

Starts the stdio-only MCP adapter with twelve read-only Tua tools. It pins MCP
protocol `2025-11-25` and applies no edits.

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
TUA_OLLAMA_MODEL=qwen2.5-coder:1.5b-base make ollama-completion-smoke
```

`make verify` is non-mutating: it checks formatting, tests, strict Clippy,
extension activation, tracked JSON, and whitespace without rewriting source.
`make ci-check` adds bundled example builds, the documentation build, and the
benchmark smoke. Generated LOVE catalog verification still needs
`LOVE_API_ROOT`, while the real VS Code extension-host test remains
`make extension-test`.
