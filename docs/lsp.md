---
icon: lucide/server
---

# Language Server

Tua's language server provides compiler-backed editing for `.tua` projects.
Diagnostics, completion, hover, navigation, refactors, formatting, and project
inspectors read the same semantic analysis used by `tua check` and `tua build`.

!!! tip "Use the VS Code extension"
    The recommended setup is the [Tua VS Code extension](installation.md).
    Published packages include the matching server binary and start it
    automatically, so a separate CLI installation is not required.

The standalone server runs over standard input and output:

```bash
tua lsp
```

`tua lsp --stdio` is also accepted for editor-client compatibility. The server
speaks JSON-RPC/LSP on stdout, so clients must not treat it as an interactive
terminal command.

## Find A Feature

| Goal | Feature |
| --- | --- |
| Understand a type or `any` result | Hover, inlay hints, and **Tua: Explain Type / Why Is This Any?** |
| Complete Lua, Tua, LOVE, modules, or assets | Contextual completion, snippets, and completion resolve |
| Move through a project | Definition, type definition, implementation, references, symbols, call hierarchy, and module explorer |
| Change code safely | Rename, quick fixes, source actions, and file-rename import updates |
| Inspect a LOVE project | Asset, Signal, Module, ECS, Object Pool, and Sprite tools |
| Inspect generated output | **Tua: Open Generated Lua Beside Tua** and source-map navigation |
| Diagnose server problems | Tua output, language status, server health, traces, and debug information |

## Everyday Editing

### Diagnostics And Incomplete Code

The server publishes parser, type, project, lint, asset, LOVE, and domain
diagnostics with stable `TLxxxx` codes. They match `tua check` for the same
project snapshot. See [Diagnostic Codes](luahelper-code-matrix.md) for
code definitions and [Configuration](configuration.md#diagnostics) for project
settings.

Standard lint rules are published by default. Set `diagnostics.rules = false`
(or use the `tua.diagnostics.rules` override) to disable them.
Formatting differences are not diagnostics; editor formatting and
`tua fmt --check` use the separate formatter.

During typing, Tua reports fast current-file diagnostics first and follows with
project-aware results after a short debounce. Requests are tied to document
versions; work from an older edit is cancelled or discarded instead of being
published over newer text.

Open documents retain their last valid semantic facts while syntax is
temporarily incomplete. Completion uses the current cursor context with those
facts, while hover and inlay hints remain useful until valid project analysis
is available again.

Optional inline suppressions use the same syntax as `tua check`:

```tua
-- tua-ignore-next-line TL4018
player.setFilter("nearest")

local legacy: number = "soon" -- tua-ignore-line TL1003
```

Enable them with `[diagnostics].inline_ignores` in `tua.toml` or
`tua.diagnostics.inlineIgnores` in VS Code. Invalid codes and suppressions that
match no diagnostic still produce warnings. A suppression hides a diagnostic;
it does not change inference or generated Lua.

### Completion, Hover, And Signature Help

Completion combines syntax context with local and project semantics. It covers:

- locals, parameters, functions, classes, fields, aliases, arrays, and module
  exports;
- Lua builtins and supported LuaDoc tags, types, and generated annotations;
- `require(...)` and static `import(...)` paths, module members, and
  auto-import candidates;
- LOVE globals, object methods, enums, callbacks, and asset paths;
- expected-type table constructors, including union discriminators, recursive
  array/table members, and contextually typed callbacks from configured
  declaration libraries;
- table configuration for sprites, state machines, stores, ECS, object pools,
  Signals, and other Tua helpers;
- statement snippets for `if`/`then`/`end`, loops, `repeat`/`until`, `do`/`end`,
  functions, classes, and common game-helper workflows.

Normal `require(...)` and non-relative static-import path completion use the
same ordered workspace require roots as module resolution. Child-relative
imports continue to complete from the current file's directory.

Function and method items can insert argument placeholders. Known literal
choices become snippet choices, and LOVE object methods prefer colon-call
syntax. Completion resolve adds documentation, signatures, source context, and
LOVE wiki links when available.

Configured declaration-only libraries are indexed during workspace reindexing
and refreshed for file changes reported by the editor.
Their `---@meta` modules can describe the runtime module returned by a different
`require(...)` prefix, without copying the declarations into a build or
fetching packages from the editor. See
[Declaration-Only Type Libraries](configuration.md#declaration-only-type-libraries).

Hover reports inferred or declared types, documentation, module context, asset
metadata, and LOVE documentation. Short types stay inline, while complex tables,
unions, callbacks, tuples, and dictionaries use width-aware multiline formatting.
Signature help appears after `(` and `,`, re-evaluates at `)`, and supports
overloads, optional parameters, typed varargs, multiple returns, callback aliases,
and parameter documentation.

Static `stateMachine({...})` calls share one derived model across completion,
hover, signature help, checking, and inlay hints. See
[State Machines](state-machines.md) for the generated event and callback
surface.

Static `store({...})` calls likewise share one inferred state model across
completion, hover, signature help, checking, and module exports. Keyed `watch`
signatures are derived from that model. See [State Stores](stores.md).

Static `input({...})` calls derive Baton control and pair names once for
completion, hover, signature help, checking, inlay hints, and module exports.
Source strings complete LOVE keys, scancodes, gamepad axes, buttons, and hat
directions. See [Input](input.md).

!!! info "Experimental completion stays local"
    **Tua: Toggle Experimental Completions** controls contextual sequence
    ranking and validated multi-token suggestions for the workspace. Both are
    off by default and local-only. Tua sends no project source, completion
    history, or ranking data to a network service.

### Navigation And Symbols

Tua supports definition, type definition, implementation, references, prepare
rename, rename, document highlights, document symbols, workspace symbols,
document links, call hierarchy, and reference code lenses.

Cross-file navigation uses the project module graph for `.tua` and indexed
`.lua` modules. Known aliases, exported types, class members, LuaDoc symbols,
LOVE builtins, and static asset strings navigate to their source or generated
stub. Dynamic dispatch through `any`, runtime-built tables, or unresolved
modules returns no speculative location.

Renaming a source or asset file can update unambiguous static `require(...)`,
`import(...)`, and LOVE asset paths through LSP file-operation edits.

### Inlay Hints And Highlighting

The server supplies inferred type hints and parameter-name hints. Use
**Tua: Select Inlay Hint Mode** to choose:

- **Full** for all server hints;
- **Compact** for VS Code-truncated hints;
- **Types Only** or **Parameters Only**;
- **Off** for Tua files.

The extension combines immediate TextMate highlighting with compiler semantic
tokens. Semantic tokens distinguish types, classes, enums, functions, methods,
parameters, variables, properties, namespaces, events, labels, and annotations,
including declaration, readonly, deprecated, modification, and default-library
modifiers.

Full-document, range, and delta token requests use the same classification.
Lossy semantic highlighting keeps useful categories during temporary syntax
errors. The active theme controls colors; use **Developer: Inspect Editor Tokens
and Scopes** to inspect the token at the cursor.

Themes that render variables and properties identically can override only Tua
properties:

```json
"editor.semanticTokenColorCustomizations": {
  "rules": {
    "property:tua": "#4EC9B0"
  }
}
```

### Formatting

The server provides full-document, range, and on-type formatting. Invalid
documents are not reformatted. Range formatting keeps edits inside the selected
range, while on-type formatting limits edits to stable local changes after
newlines and common closing syntax.

Project defaults come from `tua.toml`:

```toml
[format]
indent_width = 2
line_width = 100
quote_style = "preserve"
final_newline = true
```

VS Code can override them with `tua.format.indentWidth`,
`tua.format.lineWidth`, `tua.format.quoteStyle`, and
`tua.format.finalNewline`. A `null` editor value inherits the project setting.

### Refactors And Code Actions

Quick fixes and source actions include:

- fixing safe diagnostics in the current file;
- removing unused local values and function declarations when their complete
  source range is compiler-proven;
- organizing, sorting, and removing compiler-resolved unused static imports
  while preserving dynamic and shadowed calls;
- changing known LOVE object calls from `.` to `:`;
- generating missing core LOVE callbacks;
- adding inferred local annotations;
- extracting expressions to locals or parameterized local functions;
- inlining stable local bindings;
- generating named table types from stable local tables;
- wrapping repeated zero-argument constructors in an `objectPool` helper.

Tua only offers a transformation when the compiler can establish a safe source
range and identity. Dynamic cases remain unchanged.

### Editing `tua.toml`

The VS Code client sends `tua.toml` documents to the same server. Configuration
files receive section and key completion, enum and default-value completion,
hover documentation, diagnostics, and unknown-key quick fixes. They are handled
as project configuration rather than parsed as Tua source.

See [Configuration](configuration.md) for every option and its default.

## LOVE And Project Tools

### LOVE Authoring

The built-in LOVE catalog provides API completion, hover, signatures,
overloads, object return types, enum values, and callback declaration snippets.
Callbacks are checked against their known signatures while still allowing
unused trailing Lua parameters to be omitted. Close callback-name typos can
offer a rename fix.

Known object methods prefer `:`. Calling one with `.` reports `TL4018` because
the receiver would not be passed as `self`:

=== "Problem"

    ```tua
    local player = image "assets/player.png"
    player.setFilter("nearest")
    ```

=== "Fix"

    ```tua
    local player = image "assets/player.png"
    player:setFilter("nearest")
    ```

`love.conf(t)` completion includes known window and module fields plus literal
choices such as fullscreen modes.

### Assets And Sprites

Static asset paths in Tua helpers and supported native LOVE constructors receive
completion, validation, hover metadata, definition, references, and rename.
Diagnostics cover missing files, unsupported formats, incompatible loader
types, and filesystem casing mismatches. Image metadata such as PNG dimensions
is shown when it can be read cheaply.

The **Tua Assets** explorer groups indexed files by kind and provides preview,
open, references, rename, and reveal actions. Unused-asset `TL2009` hints are
conservative because dynamically loaded files remain valid.

`spriteSheet({...})` receives nested configuration completion and shared
validation for image paths, grid geometry, frame ranges, durations, and loop
callbacks. Set `tua.preview.enabled` to use the experimental
**Tua: Preview Sprite Animations** webview.

### Project Explorers

The extension renders project facts returned by compiler-backed requests. It
does not parse Tua or resolve modules and assets independently.

| Tool | Shows |
| --- | --- |
| **Tua Assets** | Assets grouped by kind, metadata, static references, and conservative unused hints |
| **Tua: Show Signal Graph** | Typed events, buses, payloads, listeners, emitters, conflicts, and source navigation |
| **Tua: Show Module Dependency Graph** | Tua/Lua modules, imports, missing or duplicate resolutions, cycles, reverse dependencies, exports, and inclusion paths |
| **Tua: Show ECS Explorer** | Components, schemas, systems, queries, access patterns, and non-diagnostic performance insights |
| **Tua: Show Object Pool Explorer** | Pool declarations, inferred item types, lifecycle calls, and conservative unreleased-acquire insights |
| **Tua: Preview Sprite Animations** | Resolved sprite frames, timing, loop behavior, and the synchronized source configuration |

Module, ECS, object-pool, and asset inventory requests run on the background
lane so completion and document edits remain responsive. Signal and sprite
inspection still use synchronized compiler snapshots rather than extension-side
source scans.

See [Entity Component Systems](ecs.md) and
[Object Pooling](object-pooling.md) for the corresponding runtime APIs.

### Generated Lua And Running LOVE

**Tua: Open Generated Lua Beside Tua** opens the emitted file for the active
saved source. It uses the nearest `tua.toml` source, output, and `main` settings;
it does not build automatically.

**Tua: Jump Between Tua and Generated Lua** uses the emitted
`*.lua.map.json` sidecar to move between source and output lines. Current source
maps provide line provenance, so target columns begin at column zero.

**Tua: Run LOVE Project** builds and launches LOVE from the output directory in
a visible, cancellable VS Code task. **Tua: Restart LOVE Project** replaces the
tracked task. `print(...)`, stdout, stderr, and mapped runtime traceback
locations appear in the task terminal and the `tua` output channel. Configure a
non-standard LOVE executable with `tua.love.command`.

**Tua: Debug LOVE Project** provides source-mapped LOVE debugging through an
external Lua adapter. See [Debugging](debugging.md) for setup, runtime behavior,
and launch guidance.

## VS Code Commands

Open the command palette and type **Tua**. The most useful commands are:

| Command | Purpose |
| --- | --- |
| **Tua: Initialize Tua Project** | Create the minimum `tua.toml` and source layout without overwriting existing files |
| **Tua: Explain Type / Why Is This Any?** | Trace annotations, inference, narrowing, module resolution, and dynamic fallback points |
| **Tua: Select Inlay Hint Mode** | Choose full, compact, filtered, or disabled hints |
| **Tua: Open Generated Lua Beside Tua** | Inspect emitted Lua for the active source |
| **Tua: Jump Between Tua and Generated Lua** | Navigate through the generated source map |
| **Tua: Run LOVE Project** / **Restart LOVE Project** | Build and run LOVE in a VS Code task |
| **Tua: Debug LOVE Project** | Build and debug generated Lua with breakpoints mapped to Tua |
| **Tua: Workspace Symbols by Module** | Browse compiler symbols grouped by module |
| **Tua: Why Is This Module Included?** | Show the active module's shortest root-to-module path |
| **Tua: Organize Imports** / **Sort Imports** / **Remove Unused Imports** | Apply compiler-backed source actions |
| **Tua: Fix All Safe Diagnostics** | Apply safe fixes in the active file |
| **Tua: Toggle Experimental Completions** | Toggle the two top-level experimental completion lanes |
| **Tua: Focus Output** | Open server, build, and runtime logs |
| **Tua: Show Language Server Health** | Inspect revisions, queues, cache rates, warming, stale results, and request latency |
| **Tua: Restart Language Server** | Restart the current bundled or selected server |
| **Tua: Select Server** | Choose the bundled server, a local executable, or a configured path |
| **Tua: Copy Debug Info** / **Report Issue** | Collect version and workspace context for a bug report |

Context menus and explorer views add symbol, hover, asset, and graph-specific
actions when applicable.

## Settings

Project behavior belongs in `tua.toml`; editor-specific behavior belongs in VS
Code settings. Nullable language settings override the project only when they
contain a value.

| Setting | Purpose |
| --- | --- |
| `tua.server.path` | Select the `tua` executable used by the extension |
| `tua.love.command` | Select the LOVE executable used by run tasks |
| `tua.trace.server` | Show `off`, `messages`, or `verbose` protocol traces |
| `tua.log.verbosity` | Show `off`, `error`, `info`, `debug`, or `trace` server logs |
| `tua.format.*` | Override formatter settings from `tua.toml` |
| `tua.diagnostics.*` | Override individual diagnostic, inline-ignore, and accepted-global settings |
| `tua.jit.enabled` | Enable conservative LuaJIT shape guidance; disabled by default |
| `tua.ecs.enabled` | Enable or disable built-in ECS analysis; enabled by default |
| `tua.completion.sequenceRanking.*` | Configure experimental contextual completion ranking |
| `tua.completion.multiToken.*` | Configure experimental validated multi-token suggestions |
| `tua.fileAssociations` | Index additional glob patterns as Tua or Lua source |
| `tua.ignoredPaths` | Exclude matching paths from indexing and diagnostics |
| `tua.requirePathSeparator` | Suggest module paths with `.` or `/` separators |
| `tua.preview.enabled` | Enable experimental visual previews; disabled by default |

Trace and log settings update without a restart when supported. Changing the
server path restarts the client with the selected executable.

## Other LSP Clients

Point an LSP client at `tua lsp` and use the workspace containing `tua.toml` as
the root. A client should send incremental document synchronization, workspace
folder changes, configuration changes, and watched-file events for `.tua`,
indexed `.lua`, assets, and `tua.toml` when supported.

### Standard Protocol Surface

| Area | Supported requests |
| --- | --- |
| Synchronization | Open, incremental change, save with text, and close |
| Diagnostics | Push diagnostics and pull diagnostics when the client advertises support |
| Writing | Completion with resolve, hover, and signature help |
| Navigation | Definition, type definition, implementation, references, document highlight, links, symbols, workspace symbols, and call hierarchy |
| Editing | Prepare rename, rename, quick fixes, source actions, code lenses, selection ranges, and folding ranges |
| Presentation | Inlay hints and full, range, and delta semantic tokens |
| Formatting | Full-document, range, and on-type formatting |
| Workspace | Workspace folders, watched files, and will-rename file edits |

### Tua Extension Requests

The bundled VS Code extension also uses version-matched Tua requests:

| Method | Result |
| --- | --- |
| `tua/explainType` | Source-linked type inference explanation |
| `tua/inspect` | Structured document inspector data, including sprite sheets |
| `tua/assetInventory` | Project assets, metadata, references, and hints |
| `tua/signalGraph` | Typed SignalBus project graph |
| `tua/moduleGraph` | Module dependency and export graph |
| `tua/ecsGraph` | ECS components, systems, queries, and insights |
| `tua/objectPoolGraph` | Object-pool declarations and lifecycle facts |
| `tua/builtinDocument` | Generated Lua and LOVE builtin stub source |
| `tua/cacheStats` | Analysis cache and workspace-warming counters |
| `tua/health` | Server revision, queue, cache, stale-result, and latency health |
| `tua/resetCompletionStats` | Clear local adaptive completion statistics |

`tua/setTrace` updates runtime trace configuration. These requests are an
extension integration surface rather than a general stable external API; keep
custom clients matched to the server version. The constraints for a future
versioned tool API are documented in
[External API Bridge](external-api-bridge.md).

## Troubleshooting

1. Open the folder containing `tua.toml`, then confirm the file's language mode
   is **Tua**.
2. Check the Tua language status item for server state, path, version, and
   workspace.
3. Run **Tua: Focus Output** for startup, build, protocol, and runtime messages.
4. Run **Tua: Explain Type / Why Is This Any?** when completion disappears
   because a value became dynamic.
5. Run **Tua: Show Language Server Health** when requests feel stale or slow.
6. Use **Tua: Restart Language Server** after changing a development binary.
7. Use **Tua: Select Server** or set `tua.server.path` if the bundled binary is
   unavailable or a local build is required.
8. Use **Tua: Copy Debug Info** before filing an issue.

Set `tua.trace.server` to `messages` or `verbose` for protocol traces and
`tua.log.verbosity` to `debug` or `trace` for server diagnostics. Avoid leaving
verbose tracing enabled during normal use because the output is intentionally
high volume.

If generated Lua is missing, run **Tua: Run LOVE Project** or `tua build` before
using generated-source commands. If sprite preview is unavailable, enable
`tua.preview.enabled`; ordinary sprite diagnostics and completion do not depend
on the preview setting.

## Contributor Checks

Use focused checks for language-server changes:

```bash
cargo test -p tua-lsp
make lsp-smoke
make lsp-features-smoke
npm --prefix editors/vscode run test:vscode
```

The extension-host suite covers live completion freshness, cancellation,
navigation, signature help, inlay hints, UTF-16 positions, file-renames,
project explorers, asset actions, and sprite preview behavior.

For performance work, start with:

```bash
make profile-lsp
make profile-completion
make profile-representative
```

See [Completion Benchmarks](completion-benchmarks.md) for benchmark selection,
latency metrics, baselines, cache interpretation, and freshness checks. Set
`TUA_PROFILE=1` when internal analysis spans are needed; normal server output
remains clean when profiling is disabled.
