---
icon: lucide/terminal
---

# CLI

Every command has contextual help. For example, `tua init --help` describes
only project initialization, while `tua --help` lists the complete command
set.

Run these commands from the project folder containing `tua.toml`. Install the
[standalone CLI](installation.md#standalone-cli-release) to use `tua` in a shell;
the VS Code extension's bundled server is not added to your `PATH`.

| Task | Command | Writes files? |
| --- | --- | --- |
| Create the project layout | `tua init` | Creates missing project files. |
| Check types and project diagnostics | `tua check` | No Lua output; configured asset pipelines may generate their outputs. |
| Execute unit tests with Busted | `tua test` | Writes temporary test output; configured asset pipelines may generate files. |
| Apply formatting | `tua fmt` | Rewrites project Tua source. |
| Check formatting in CI | `tua fmt --check` | No. |
| Run lint rules | `tua lint` | No. |
| Build Lua and copy assets | `tua build` | Writes the configured output directory. |
| Rebuild after saves | `tua watch` | Writes build output; does not launch LOVE. |
| Play with hot reload | `tua dev` | Builds and starts LOVE. |
| Produce a distributable game | `tua package` | Builds and writes `dist/`. |

Repository contributors can substitute `cargo run -p tua-cli --` for `tua`.
See [Configuration](configuration.md) for paths and [Troubleshooting](troubleshooting.md)
for failed checks, missing commands, and runtime errors.

## `init`

```bash
tua init
```

Creates `tua.toml`, `src/`, and `src/main.tua` if they do not already exist. It
also creates or extends `.gitignore` with the generated `build/`, `dist/`, and
`.tua/` directories without removing existing entries.
Initialization rejects symlinked destinations, including `src` and `.gitignore`,
before creating project files.

## `check`

```bash
tua check
```

Loads project config, parses sources, runs type checking and configured project
diagnostics, and prints them. Standard lint rules are enabled by default; set
`diagnostics.rules = false` for a type/project-only check. The command exits
non-zero for errors and still prints non-blocking warnings, information, and
hints. Use `tua check --deny-warnings` when CI should reject warnings too.

## `test`

```bash
tua test
tua test spec/counter_spec.tua
tua test --filter independent
tua test --busted /path/to/busted --lua luajit
```

Builds fresh temporary output and runs `_spec` files under the configured source
root with an installed Busted backend. The optional path is source-root-relative.
Failures map back to Tua source and exit nonzero; the normal build output remains
unchanged. See [Testing Tua](testing.md) for setup, tags, editor usage, and limits.

## `fmt`

```bash
tua fmt
tua fmt --check
```

Formats project `.tua` source files in place using the built-in formatter.
`--check` reports files that would change and exits non-zero without writing.
Files with parse errors are left unchanged and reported with diagnostics.
Formatting does not produce lint diagnostics; see [Formatter](formatter.md).

## `lint`

```bash
tua lint
tua lint --strict
```

Runs syntax diagnostics and the standard lint rules, reporting success as
`linted N file(s)`. Type, project, and domain diagnostics remain the
responsibility of `tua check`. `--strict` additionally enables every broadly
applicable Tua-specific optional lint while keeping noisy project-policy rules
such as direct-call-only function use, complexity, literal-zero division,
failed swaps, and mixed tables opt-in. Inline-ignore settings remain controlled
by project/editor configuration. Lint exits non-zero for warning or error
diagnostics; information and hints remain advisory. See [Linter](linter.md) for
rule compatibility.

## Diagnostic Timings

Set `TUA_TRACE=diagnostics` to print one stderr timing line for `check` and
`lint` diagnostic work:

```bash
TUA_TRACE=diagnostics tua check
TUA_TRACE=diagnostics tua lint
```

The trace reports `total`, `covered`, `unaccounted`, and fixed buckets for
`discover_files`, `read_vfs_update`, `parse`, `module_graph`,
`type_environment`, `checker`, `lints`, `inline_ignores`, and `publish`. Each
bucket is printed as milliseconds/count, for example `parse=2.413ms/21`.

## `build`

```bash
tua build
```

Checks the project and writes emitted Lua to the configured output directory.
Emission is blocked by errors by default.

Configured [asset pipelines](asset-pipelines.md) run or restore their cached
outputs before source discovery, so generated `.tua` and `.d.tua` modules use
the same checking and emission path as handwritten source.

Build output mirrors the configured source tree. The configured `main` source
file also emits to `<out>/main.lua` for LOVE entrypoint loading while keeping
its normal mirrored output path.

Separate-output builds record generated files in `.tua-build-manifest.json`.
After a successful rebuild, obsolete recorded outputs are removed only if their
contents are unchanged. Unrecorded and manually modified files are preserved on
disk; packages include only the current build inventory. In-place builds and
overlapping source/output trees do not perform this cleanup. Old outputs from
before the first manifest are preserved on disk but excluded from packages.

- `.tua` files are checked, lowered, erased, and emitted as `.lua`;
- plain `.lua` files and other assets are copied unchanged;
- bundled helper runtimes such as `brinevector.lua`, `anim8.lua`,
  `tuasignal.lua`, `tuaecs.lua`, `tuaobjectpool.lua`, `tuastatemachine.lua`,
  `tuastore.lua`, `tuaaudio.lua`, `baton.lua`, `tuatween.lua`,
  `tuacamera.lua`, `tuarot.lua`, and `tuabehavior.lua` are written only when generated Lua or a
  copied `.lua` source requires them with a static string literal. The audio
  runtime also brings in its `tuasignal.lua` and `tuastore.lua` dependencies;
- in-place builds are supported when `source` and `out` are the same directory;
- collisions between generated `.lua` and sibling/copied `.lua` files are
  reported as warnings.

For a separate output directory, a later build refreshes a bundled helper when
the build manifest still identifies its existing file as unchanged output from
Tua. A helper supplied in the source tree, or an output helper changed by hand,
is preserved. Older output with no recorded ownership is also preserved; remove
that stale helper from the output directory to let Tua write the current one.
In-place builds likewise preserve an existing helper because it may be project
source. Keep custom implementations in the source tree rather than editing a
generated helper in the output directory.

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
    tua build --emit-on-error
    ```

## `package`

```bash
tua package
tua package --target windows --love path/to/love-11.5-win64
tua package --target macos --love /Applications/love.app
tua package --target web --web-player path/to/love.js
```

Builds the project, then writes `dist/<name>.love`. `--target windows` or
`--target macos` fuses a standalone game with the LOVE runtime passed to
`--love`; `--target web` pairs the archive with a supplied LÖVE 11.5 web
player. Packaging never downloads anything. `--name` sets the game name
(default: the project directory's name) and `--identifier` the macOS bundle
identifier. See [Packaging Games](packaging.md).

## `traceback`

```bash
tua traceback --maps build sentry-traceback.txt
cat sentry-traceback.txt | tua traceback --maps build
```

Remaps generated Lua traceback locations back to Tua source using
`*.lua.map.json` sidecars from `tua build`. If `--maps` is omitted, the command
uses the configured output directory.

For example, `build/main.lua:12` can become:

```text
src/main.tua:18 (generated build/main.lua:12)
```

Ambiguous basename-only frames are left unchanged instead of guessed.

## `watch`

```bash
tua watch
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
tua dev
tua dev --love /path/to/love
tua dev --no-launch
```

Builds like `watch`, starts LOVE on the output, and hot reloads the running
game after every error-free rebuild. `--love` or `TUA_LOVE` chooses the LOVE
executable; otherwise `love` on `PATH` is used, then `/Applications/love.app`
on macOS. Closing the game ends the command. See [Hot Reload](hot-reload.md).

Use `--no-launch` when an editor manages the game processes. It prepares the same
hot-reload runtime and signal file, prints `dev: ready <output-directory>` only
after the first successful build, and keeps watching without launching LOVE.
Multiple game processes can share that output and reload after each successful
build. The caller owns their lifetimes and must stop the watcher when finished.

## `lsp`

```bash
tua lsp
tua lsp --stdio
```

Starts the language server over stdio. Editors and tests call this command
directly. `--stdio` is accepted for editor-client compatibility and has the same
behavior as `tua lsp`.

## `api`

```bash
tua api --stdio
tua api --stdio --workspace /path/to/project
```

Starts the read-only, newline-delimited JSON-RPC API. It keeps one project
analysis session alive and refreshes disk state only after
`tua.api/refresh`. See [External API And MCP](external-api-bridge.md).

## `mcp`

```bash
tua mcp --stdio
tua mcp --stdio --workspace /path/to/project
```

Starts the stdio-only MCP adapter with twelve read-only Tua tools. It pins MCP
protocol `2025-11-25` and applies no edits.

## Shader Compilation Checks

`tua check` uses LÖVE's `love.graphics.validateShader` for statically referenced
shader assets. `TL2016` identifies the shader line and its loader calls. This runs
Tua's bundled validator in a fresh temporary directory, without loading your game.
Build, watch, dev, and lint do not run this compilation check.

Use `tua check --love /path/to/love`, `TUA_LOVE`, or put LÖVE on `PATH`. On macOS
Tua also tries `/Applications/love.app/Contents/MacOS/love`. On Windows use
`lovec.exe` for console I/O; automatic discovery prefers it. Windows validation
has not yet been tested on a real LÖVE installation. A graphical session and
working graphics driver are required for LÖVE. In `auto` mode, failure to start
or communicate with LÖVE falls back to headless `glslangValidator`. Actual shader
errors from LÖVE are retained. Find glslang on `PATH` or set `TUA_GLSLANG` to a
trusted executable. If neither tool exists, Tua prints one skip note (unaffected
by `--deny-warnings`); other validator failures fail the check.

The fallback uses generated headers from vendored LÖVE 11.5, with gamma correction
disabled. Desktop uses GLSL 3.30; GLES uses ES 1.00 for `glsl1` and ES 3.00 for
`glsl3`. It validates language compatibility, not a device's GPU support.

```toml
[shaders]
validate = "auto" # also "love", "glslang", or "off"
targets = ["desktop", "gles"] # default: ["desktop"]
```

Executable paths are deliberately not accepted in project configuration. Each
validation batch is limited to 512 requests, 16 MiB of shader source, and ten
seconds per backend after startup (LÖVE startup also has a ten-second timeout).
LÖVE validation reflects the installed engine; glslang follows the bundled 11.5
wrapper. Neither proves support on every device.

<span id="check-shader-validator-installation"></span>

## `shader-tools`

Run `tua shader-tools` to test LÖVE and glslang with a built-in shader and
print a JSON report (`love.canValidate`, `love.version`, and
`glslang.canValidate`). This uses isolated temporary files, never loads the
project, and can briefly open LÖVE’s graphics window. Each backend is bounded
by a timeout; unavailable tools are reported in JSON rather than as command
failure. `TUA_LOVE` and `TUA_GLSLANG` select trusted executables.
