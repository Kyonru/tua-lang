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

Editor request bursts wait in a bounded queue when worker slots are occupied.
Interactive requests retain priority over background refreshes; cancelled or
outdated queued requests are discarded before analysis starts. Capacity errors
can still occur at the overall limit of 256 outstanding client requests.
The coordinator checks newly arrived input between queued-job dispatches so
background refreshes do not fill worker slots in one uninterrupted batch.

!!! info "Everything runs locally"
    The server and extension send no source code or completion-learning data
    off your machine. Completion is computed locally, and the
    model-integration setting does not enable any network-backed provider.
    Model-based inline completion is opt-in and uses a model running on your
    own machine through [Ollama](#local-ollama-inline-completion).

## Find A Feature

| Goal | Feature |
| --- | --- |
| Understand a type or `any` result | Hover, inlay hints, and **Tua: Explain Type / Why Is This Any?** |
| Complete Lua, Tua, LOVE, modules, or assets | Contextual completion, snippets, and completion resolve |
| Move through a project | Definition, type definition, implementation, references, symbols, call hierarchy, and module explorer |
| Change code safely | Rename, quick fixes, source actions, and file-rename import updates |

Symbol rename conservatively refuses a replacement name already used in any
affected file, including free/global references that a new local could capture.
Choose a fresh name when scopes are ambiguous. This can also refuse otherwise
safe reuse in disjoint scopes.
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

Nested `TL1003` errors identify the failing contract path, such as
`config.animations.green.row: expected number, found string`, and highlight the
corresponding table value when it is statically available. Root-level failures
retain concise declared names such as `Player`.

Standard lint rules are published by default. Set `diagnostics.rules = false`
(or use the `tua.diagnostics.rules` override) to disable them.
Undefined-variable `TL4004` diagnostics are errors in Tua sources, so editors
show misspelled or undeclared names as red file problems. They remain warnings
in plain Lua sources, where runtime-provided globals are common; projects can
declare accepted globals through `diagnostics.globals`.
Formatting differences are not diagnostics; editor formatting and
`tua fmt --check` use the separate formatter.

During typing, Tua reports fast current-file diagnostics first and follows with
project-aware results after a short debounce. Requests are tied to document
versions; work from an older edit is cancelled or discarded instead of being
published over newer text. When imports or required modules can change a type
answer, the fast pass defers type mismatches to that project-aware pass rather
than briefly showing a stale red squiggle.

Open documents retain their last valid semantic facts while syntax is
temporarily incomplete. Completion uses the current cursor context with those
facts on both manually invoked and trigger-character requests, while hover and
inlay hints remain useful until valid project analysis is available again.
Parse-independent Lua, LOVE, keyword, snippet, and module-path candidates stay
available even when no valid semantic snapshot exists yet. Ordinary type
errors do not reduce completion availability.

Exact-code inline suppressions use the same syntax as `tua check` and are
enabled by default:

```tua
-- tua-ignore-file TL4048

-- tua-ignore-next-line TL4018
player.setFilter("nearest")

local legacy: number = "soon" -- tua-ignore-line TL1003
```

The file form must be the first nonblank line. Empty directives and broad
category suppressions are invalid. Projects can disable source suppressions
with `[diagnostics].inline_ignores = false` in `tua.toml` or
`tua.diagnostics.inlineIgnores` in VS Code. Invalid codes and suppressions that
match no diagnostic still produce warnings. A suppression hides a diagnostic;
it does not change inference or generated Lua.

For diagnostics with a suppressible exact code, the editor offers
`Suppress TLxxxx for this line` and `Suppress TLxxxx for this file` quick
fixes. The line action uses a same-line comment when possible and preserves an
existing trailing comment by inserting a next-line directive instead. The file
action extends an existing header directive when present. These actions follow
the project's enabled scopes and are absent when inline ignores are disabled.

### Completion, Hover, And Signature Help

Completion combines syntax context with local and project semantics. It covers:

- locals, parameters, functions, classes, fields, aliases, arrays, and module
  exports;
- exported project types, with automatic `import type` insertion and
  unknown-type import quick fixes;
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

Type auto-imports are offered only in type positions. They reuse existing
named imports and namespaces, recognize equivalent specifiers for the same
module, and include aliases exposed through type re-exports.

Configured declaration-only libraries are indexed during workspace reindexing
and refreshed for file changes reported by the editor.
Their `---@meta` modules can describe the runtime module returned by a different
`require(...)` prefix, without copying the declarations into a build or
fetching packages from the editor. See
[Declaration-Only Type Libraries](configuration.md#declaration-only-type-libraries).

Hover reports inferred or declared types, documentation, module context, asset
metadata, and LOVE documentation. Short types stay inline, while complex tables,
unions, callbacks, tuples, and dictionaries use width-aware multiline formatting.
For an indexed assignment, hover reports the collection, index, and assigned
value independently; the assignment operator itself is not presented as a
comparison expression.
Nested fields and callback parameters inside built-in helper configurations such
as `spriteSheet({...})`, `stateMachine({...})`, and `objectPool({...})` use their
expected schema instead of collapsing to the enclosing call's result type.
Signature help appears after `(` and `,`, re-evaluates at `)`, and supports
overloads, optional parameters, typed varargs, multiple returns, callback aliases,
and parameter documentation.

Static `stateMachine({...})` calls share one derived model across completion,
hover, signature help, checking, inlay hints, inspection, and the experimental
source-linked graph preview. See
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
    ranking and compiler-generated multi-token suggestions for the workspace.
    Both are off by default.

    **Tua: Toggle Deterministic Inline Completions** controls source-only ghost
    text for the workspace. It is also off by default and does not enable
    Ollama.

    **Tua: Toggle Local Ollama Completions** separately opts into model-backed
    inline ghost text. The extension sends a bounded prefix, suffix, language
    contract, and visible type facts directly to a configured loopback Ollama
    server, then asks Tua to validate the candidate in a cloned analysis host.
    Normal completion-menu results never go through Ollama.

### Deterministic Local Inline Completion

Run **Tua: Toggle Deterministic Inline Completions**, or set
`tua.completion.inline.enabled` to `true`, to try deterministic ghost text
without a model. The command changes only that workspace gate. The feature is
experimental and off by default. The public extension configuration has three
independently configurable sources:

- `tua.completion.inline.sources.semantic` reuses unique, source-only statement
  snippets after at least three prefix characters;
- `tua.completion.inline.sources.currentFile` recalls conservative
  continuations from other lines in the active file while preserving their
  original spelling and layout;
- `tua.completion.inline.sources.statistical` opts into the experimental,
  Tua-only project statistical fallback. It remains off by default and runs
  only after deterministic sources abstain.

Current-file recall excludes the line being edited, requires a meaningful
prefix, and stops at the end of the line. A continuation supported by only one
other line is shortened to its first token; longer common prefixes require at
least two supporting lines. Recall is currently suppressed in assignment and
member-access contexts, where the tuning data was substantially less precise.
Semantic statement snippets may contain their normal block body, but the
pattern-recall source is single-line.

The server returns at most one suggestion and abstains when the cursor, suffix,
document size, parse state, deadline, or cancellation state makes the result
unsafe. Suggestions are version-bound, so edits and newer requests discard
stale work. When the completion menu is open, ghost text appears only if it
extends the selected item.

Local sources run before Ollama and never send code over the network. A local
timeout or cancellation returns no suggestion instead of starting model work.
The language status reports `Local`, `Ollama`, `Local + Ollama`, or `off`.
Source-specific returned and accepted counters contain no completion text and
do not affect ranking.

Standalone clients can use the standard LSP 3.18
`textDocument/inlineCompletion` request. The VS Code extension combines local
and Ollama suggestions through one provider to avoid duplicate ghost text.

### Local Ollama Inline Completion

Install Ollama separately, then pull the default model:

```bash
ollama pull qwen2.5-coder:1.5b-base
```

Run **Tua: Test and Warm Ollama Model** to preload it. Enable the feature with
**Tua: Toggle Local Ollama Completions** after reviewing the source-sharing
consent message. Tua never installs or pulls a model.

The first release accepts unauthenticated `http://` endpoints only on
`localhost`, `127.0.0.0/8`, or IPv6 loopback. It keeps one Ollama request in
flight and cancels it on edits, cursor movement, configuration changes,
provider cancellation, or a newer request. Requests are skipped in comments,
strings, multi-cursor sessions, and while a normal completion item is selected;
the deterministic backend may still extend that selected item. These expected
cancellations and document-version races are counted locally but do not produce
warnings. Completion status distinguishes Tua language-server availability
from Ollama transport, missing-model, and timeout failures.

Tua classifies each request as a local or function completion. Local expression
and statement candidates remain bounded to eight lines and 1 KiB. Inside an
unfinished function or method, Ollama may complete the remaining body and its
closing `end` or `}` with up to 64 lines and 8 KiB. The default generation
ceiling is 256 tokens, with a 5-second timeout; the function lane can use up to
512 configured tokens.

Tua removes FIM/end tokens and duplicated suffix text, then rejects Markdown,
empty output, stale fingerprints, obvious foreign-language constructs such as
TypeScript/JavaScript function syntax, and every newly introduced error. The
model prompt is itself valid commented Tua source. It teaches that ordinary
Lua/LuaJIT is the base, prefers inferred locals over redundant annotations,
includes native LOVE 11.x guidance, and carries a bounded inventory of the
implemented Tua type, class, module, interop, and game-helper features.
Cursor-specific expected types and visible bindings appear before that
inventory. Existing unrelated diagnostics and gradual `any` are preserved. The
live document and analysis host are never mutated.

The extension stores only source-free Ollama counters: request/cancellation/
timeout/empty/rejection counts, suggestions returned and accepted, and
accepted character totals. They appear alongside the deterministic counters in
copied debug information and are cleared by **Tua: Reset Completion Learning
Stats**.

The feature remains experimental and disabled by default.

### Navigation And Symbols

Tua supports definition, type definition, implementation, references, prepare
rename, rename, document highlights, document symbols, workspace symbols,
document links, call hierarchy, and reference code lenses.

Cross-file navigation uses the project module graph for `.tua` and indexed
`.lua` modules. Known aliases, exported types, class members, LuaDoc symbols,
LOVE builtins, and static asset strings navigate to their source or generated
stub. Dynamic dispatch through `any`, runtime-built tables, or unresolved
modules returns no speculative location.

Built-in helper configuration fields navigate to generated virtual schema
documents. Names derived from static source retain their source identity:
`store` watcher keys navigate to the initial state field, sprite animation
members navigate to the named animation entry, and state-machine event methods
and callback keys navigate to the event or state string that created them.

Native class names, fields, and methods support definition, references, prepare
rename, and project-wide rename. The same identity follows locally inferred
instances and nominal instances exported through a static module, so navigating
`Module.player.health` reaches the original class field rather than generated
Lua or the module's `player` export. Resolved named table-shape fields are also
renameable when Tua can prove their declaration identity.

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

Resolved `---@deprecated` declarations and uses receive the deprecated modifier.
Required-module completion items use the standard LSP deprecated tag, and hover
shows any replacement guidance carried by the annotation.

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
- suppressing one exact diagnostic code for its line or for the file; these
  explicit actions are excluded from safe fix-all;
- manually renaming a duplicate table key when the intended replacement is
  known; this intent-dependent edit is excluded from safe fix-all;
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
callbacks. Static sheets also complete exact runtime animation, marker, and
child names. The inspector carries ordered multi-row ranges, nonuniform frame
durations, registration origins, trim/source metadata, markers, and child
descriptors from the same compiler model. Set `tua.preview.enabled` to use the experimental
**Tua: Preview Sprite Animations** webview.

Static state-machine configurations use the same preview gate for
**Tua: Preview State Machine**. The graph highlights the declared initial
state, expands multi-source and wildcard event routes, follows source edits,
and navigates back to state and event declarations. Sprite sheets and state
machines link directly to their previews from hover; recognized asset-path
hovers likewise link to the native asset preview. Preview links trust only
their registered Tua commands. The graph never runs the project or claims to
show the machine's live runtime state.

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
| **Tua: Show Audio Explorer** | Static audio systems with their buses, cues, effects, music scenes, and layers |
| **Tua: Preview Sprite Animations** | Resolved sprite frames, timing, loop behavior, and the synchronized source configuration |
| **Tua: Preview State Machine** | Initial state, statically derived states, source-linked event routes, wildcard transitions, and cycles |

Module, ECS, object-pool, audio, and asset inventory requests run on the background
lane so completion and document edits remain responsive. Signal, sprite, and
state-machine inspection still use synchronized compiler snapshots rather than
extension-side source scans.

See [Entity Component Systems](ecs.md) and
[Object Pooling](object-pooling.md) and [Audio Systems](audio.md) for the corresponding runtime APIs.

### Generated Lua And Running LOVE

**Tua: Open Generated Lua Beside Tua** saves the active source, runs
`tua build` with the extension's selected Tua executable from the nearest
project root, and opens or reloads the emitted file beside it. The command uses
the nearest `tua.toml` source, output, and `main` settings. A failed build does
not open stale output, and an already-open generated file with unsaved manual
edits must be saved or discarded before rebuilding.

**Tua: Jump Between Tua and Generated Lua** uses the emitted
`*.lua.map.json` sidecar to move between source and output lines. Current source
maps provide line provenance, so target columns begin at column zero.

**Tua: Run LOVE Project** builds and launches LOVE from the output directory in
a focused, cancellable VS Code task. `print(...)`, stdout, stderr, and mapped
runtime traceback locations appear in its dedicated terminal. **Tua: Stop LOVE
Project** terminates the active run, and **Tua: Restart LOVE Project** replaces
it. The editor title shows Run while idle and Stop while the game is active.
The `tua` output channel remains reserved for extension and language-server
messages. Configure a non-standard LOVE executable with `tua.love.command`.

**Tua: Run LOVE Project With Hot Reload** runs [`tua dev`](hot-reload.md) in
a task: the game reloads changed modules each time you save. **Tua: Package
Game...** asks for a `.love`, Windows, or macOS build and runs
[`tua package`](packaging.md). Fused builds use the LOVE runtime in
`tua.love.windowsRuntime` or `tua.love.macosApp`; when that is empty, the
command asks for it and can remember the choice.

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
| **Tua: Open Generated Lua Beside Tua** | Rebuild and inspect emitted Lua for the active source |
| **Tua: Jump Between Tua and Generated Lua** | Navigate through the generated source map |
| **Tua: Run LOVE Project** / **Stop LOVE Project** / **Restart LOVE Project** | Build, stop, or restart LOVE in a focused VS Code task terminal |
| **Tua: Run LOVE Project With Hot Reload** | Run the game and reload code changes while it plays |
| **Tua: Package Game...** | Build a `.love`, fused Windows, or fused macOS package |
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
| `tua.love.windowsRuntime` | Extracted LOVE for Windows folder used for fused Windows packages |
| `tua.love.macosApp` | `love.app` used for fused macOS packages |
| `tua.trace.server` | Show `off`, `messages`, or `verbose` protocol traces |
| `tua.log.verbosity` | Show `off`, `error`, `info`, `debug`, or `trace` server logs |
| `tua.format.*` | Override formatter settings from `tua.toml` |
| `tua.diagnostics.*` | Override individual diagnostic, inline-ignore, and accepted-global settings |
| `tua.jit.enabled` | Enable conservative LuaJIT shape guidance; disabled by default |
| `tua.ecs.enabled` | Enable or disable built-in ECS analysis; enabled by default |
| `tua.completion.sequenceRanking.*` | Configure experimental contextual completion ranking |
| `tua.completion.multiToken.*` | Configure experimental validated multi-token suggestions |
| `tua.completion.inline.*` | Configure experimental deterministic inline completion and its local sources |
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
| `tua/inspect` | Structured document inspector data, including sprite sheets and static state machines |
| `tua/assetInventory` | Project assets, metadata, references, and hints |
| `tua/signalGraph` | Typed SignalBus project graph |
| `tua/moduleGraph` | Module dependency and export graph |
| `tua/ecsGraph` | ECS components, systems, queries, and insights |
| `tua/objectPoolGraph` | Object-pool declarations and lifecycle facts |
| `tua/audioGraph` | Static audio-system declarations and their exact configured names |
| `tua/builtinDocument` | Generated Lua and LOVE builtin stub source |
| `tua/cacheStats` | Analysis cache, workspace-warming, and experimental pattern-corpus counters |
| `tua/health` | Server revision, queues, cache, pattern-corpus, stale-result, and latency health |
| `tua/resetCompletionStats` | Clear local adaptive completion statistics |
| `tua/localInlineCompletion` | Return a version-bound deterministic inline-completion candidate from enabled local sources |

`tua/setTrace` updates runtime trace configuration. These requests are an
extension integration surface rather than a general stable external API; keep
custom clients matched to the server version. For versioned, read-only tooling
outside the editor protocol, use the shipped stdio JSON-RPC API or MCP adapter
documented in [External API And MCP](external-api-bridge.md).

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

If generated Lua cannot be opened, inspect **Tua: Focus Output** for build
diagnostics. If sprite preview is unavailable, enable `tua.preview.enabled`;
ordinary sprite diagnostics and completion do not depend on the preview
setting.

### Shader uniform editing

For a local loaded with `shader "path.glsl"` or
`love.graphics.newShader("path.glsl")`, the first string argument of `:send`
and `:sendColor` offers uniform names with GLSL types and array lengths.
Unread uniforms rank last; `sendColor` prioritizes `vec3` and `vec4`.
Partial names and literal prefixes such as `"ripple" .. i` are supported.

Hover a name to see its declaration, shader location, expected Lua value, and
unread status. Pattern names list matching declarations. Go to Definition
opens the GLSL declaration; names declared in both stages offer both locations.
These features work in Tua and Lua without running LÖVE, and follow saved shader
changes and synchronized unsaved shader buffers. Unknown receivers and
macro-generated declarations stay unchecked. Proven uniform value mismatches report `TL2015`; see
[shader value checks](language.md).

### Shader compilation diagnostics

In trusted workspaces, Tua validates statically referenced shaders with the
installed LÖVE. A separate worker waits for 500 ms of idle time, reuses a single
validator process, and caches unchanged inputs. Interactive completion and
hover requests do not wait for compilation. Stale results are discarded.

`TL2016` appears on shader lines and loader calls. Open shader buffers are
validated before saving; saving, closing, changing references, or changing
`[shaders]` configuration refreshes the results. Missing LÖVE or validator
failures are reported in the language-server log. See [CLI shader checks](cli.md#shader-compilation-checks)
for discovery, limits, targets, and platform requirements.

Other LSP clients must send `initializationOptions.workspaceTrusted = true` to
permit external validation, and synchronize shader text documents to validate
unsaved buffers. The VS Code extension supplies trust and shader synchronization
without claiming GLSL formatting or completion. Restricted Mode never starts
the validator.
