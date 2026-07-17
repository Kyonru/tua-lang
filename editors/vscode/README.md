# tua VS Code Extension

Minimal VS Code language support for tua source files.

## Features

- Registers the `tua` language id.
- Associates `*.tua` files with the tua language mode.
- Uses the tua logo SVG for `.tua` file icons and a generated PNG copy for the
  packaged extension icon.
- Adds immediate TextMate grammar coverage for Lua and Tua declarations,
  classes, enums, function and callback types, LuaDoc annotations, module and
  asset paths, SignalBus event names, LOVE/ECS helpers, calls, members, labels,
  literals, comments, and operators.
- Adds language configuration for comments, brackets, auto-closing pairs,
  region folding, word boundaries, and Lua/Tua indentation.
- Bundles instant snippets for Lua functions, typed Tua declarations, classes,
  loops, conditionals, and common LOVE callbacks.
- Starts `tua lsp` over stdio and surfaces the language server's diagnostics,
  hover, completion, signature help, document highlights, navigation, rename,
  inlay hint, semantic token, and symbol features.
- Attaches the language server to `tua.toml` and provides schema-backed
  config key/section completion, enum/default value completion, hover
  documentation, and quick fixes for suggested unknown keys.
- Contributes Tua editor defaults for zero-delay quick suggestions in code and
  strings and enables semantic highlighting for Tua documents.
- Refines lexical scopes with standard VS Code semantic token types for
  classes, enums, types, functions, methods, parameters, variables, properties,
  namespaces, events, labels, and annotation tags. Declaration, readonly,
  deprecated, modification, and default-library modifiers remain theme-aware.
- Adds Tua command-palette actions for restarting the language server, focusing
  the output channel, selecting a server binary, copying debug information,
  copying or saving cache statistics, resetting completion learning stats,
  probing server profile artifacts, previewing `spriteSheet` animations,
  visualizing SignalBus events, module dependencies, and object-pool
  lifecycles across files, explaining why the active module is included,
  running source actions and refactors, reporting an issue, and opening Tua
  settings.
- Adds trusted hover action links for references, implementations, SignalBus
  event graphs, sprite sheet previews, and resolved asset previews.
- Adds a compiler-backed Tua Assets view to the Explorer, grouped by asset type
  and static usage, with reference navigation, file rename, reveal, refresh,
  dimensions, and conservative unused-asset diagnostics.
- Shows a Tua language status item with the server state, resolved server path,
  version, and workspace directory when available.
- Uses stable VS Code APIs and standard LSP features. Proposed/custom editor
  features such as verbose hover, source definition, multi-document highlight,
  and auto insert are intentionally not exposed until a concrete Lua/Tua workflow
  needs preview-gated behavior.

## Configuration

- `tua.server.path`: path to the `tua` executable. Defaults to `tua` on `PATH`.
- `tua.trace.server`: LSP trace level sent to the server. Supported values are
  `off`, `messages`, and `verbose`.
- `tua.log.verbosity`: language-server log verbosity. Supported values are
  `off`, `error`, `info`, `debug`, and `trace`.
- `tua.preview.enabled`: enables experimental editor previews such as
  `Tua: Preview Sprite Animations`.

VSIX packages built with `make extension-package` include the current platform's
`tua` server binary and use it by default.

Tagged releases publish separate `linux-x64`, `win32-x64`, `darwin-x64`, and
`darwin-arm64` VSIX files. Install the package matching the host platform and
verify it against the release `SHA256SUMS` manifest. The extension never
downloads a server during activation.

For extension-host development, `TUA_SERVER_PATH` overrides the setting.
Use the workspace `Run Extension (Features Example)` launch profile to open
`examples/features` with `target/debug/tua`. Use
`Run Extension (Features Example, Bundled Server)` to compare against a staged
bundled server. `Run Extension (Debug Server)` opens `examples/love-full` with
the debug server.

`Run Extension (All Examples)` enables `TUA_DEBUG_LSP=1`, which writes language
client setup details and server completion trace messages to the Tua output
channel. Automatic suggestions use VS Code's native zero-delay quick-suggestion
flow; the extension does not run a second timer or execute
`editor.action.triggerSuggest` after edits.

Run the real Extension Development Host integration suite with:

```bash
make extension-test
```

The suite drives an isolated two-root workspace against a controlled LSP
server. It covers completion freshness and cancellation, hover, definition,
signature help, inlay hints, UTF-16 and untitled documents, module and asset
file renames, module graph, ECS Explorer, Object Pool Explorer opening and
reuse, and sprite preview opening, reload, focus, missing assets, and hover
command links.

Use `Tua: Select Server` to switch between the bundled/default server and
workspace-local binaries such as `target/debug/tua` or `target/release/tua`.
Workspace-local binaries require VS Code workspace trust before they are
offered or executed.

Changing `tua.trace.server` or `tua.log.verbosity` sends `tua/setTrace` to the
running server. A restart is not required when the server supports the request.
Changing `tua.server.path` still prompts for a restart because the executable
selection is part of client startup.

If the executable is missing, activation shows a warning and links directly to
the setting.

## Commands

- `Tua: Restart Language Server`
- `Tua: Focus Output`
- `Tua: Open Settings`
- `Tua: Select Server`
- `Tua: Copy Debug Info`
- `Tua: Copy Cache Stats`
- `Tua: Save Cache Stats`
- `Tua: Reset Completion Learning Stats`
- `Tua: Save CPU Profile`
- `Tua: Save Heap Summary`
- `Tua: Open Generated Lua Beside Tua`
- `Tua: Jump Between Tua and Generated Lua`
- `Tua: Copy Fully Qualified Symbol Name`
- `Tua: Workspace Symbols by Module`
- `Tua: Explain Type / Why Is This Any?`
- `Tua: Select Inlay Hint Mode`
- `Tua: Preview Sprite Animations`
- `Tua: Refresh Asset Explorer`
- `Tua: Show Signal Graph`
- `Tua: Show Module Dependency Graph`
- `Tua: Show ECS Explorer`
- `Tua: Show Object Pool Explorer`
- `Tua: Why Is This Module Included?`
- `Tua: Organize Imports`
- `Tua: Sort Imports`
- `Tua: Remove Unused Imports`
- `Tua: Fix All Safe Diagnostics`
- `Tua: Report Issue`

`Tua: Select Inlay Hint Mode` is available from the command palette and the
Tua editor toolbar. Its quick pick offers Full, Compact, Types Only, Parameters
Only, and Off. Compact uses VS Code's native ellipsis rendering and restores
the exact prior language-specific maximum length when another mode is selected,
including an absent override. The other modes filter the language server's
complete type and parameter labels without changing `editor.inlayHints.enabled`
or unrelated VS Code preferences.

`Tua: Explain Type / Why Is This Any?` is available from the command palette
and the editor context menu. It asks the compiler for the type at the cursor
and lists the contributing annotation, initializer inference, literal widening,
control-flow narrowing, module resolution, member lookup, and dynamic fallback
steps. When a result is `any`, the quick-pick title calls that out directly.
Select a located step to open its declaration or source. Missing modules,
explicit `any`, unknown initializers, and member access through `any` are shown
as distinct causes.

`Tua: Open Generated Lua Beside Tua` is available from the command palette and
the Tua editor toolbar. It resolves the active saved `.tua` file through the
nearest `tua.toml` `source` and `out` settings, then opens the matching emitted
`.lua` file beside the source editor. If the output does not exist, run
`tua build` first.

`Tua: Jump Between Tua and Generated Lua` uses the emitted
`*.lua.map.json` sidecar to jump from a Tua line to the corresponding generated
Lua line, or from generated Lua back to the original Tua source line. Source
maps are line-based, so the command opens the nearest mapped line at column 0.

`Tua: Copy Fully Qualified Symbol Name` copies the module-qualified name at the
cursor, such as `systems.combat.damage`, or the current module name when the
cursor is not on a known symbol. `Tua: Workspace Symbols by Module` presents
standard workspace symbols grouped by compiler module path and opens the
selected symbol.

## Module Dependency Explorer

`Tua: Show Module Dependency Graph` opens a compiler-backed project view of
static `require(...)` and `import(...)` relationships. Filter the graph to
unresolved dependencies, cycles, or root candidates, select a module to inspect
its direct dependencies and reverse dependencies, and open inferred exports at
their definitions. Missing and invalid imports show the searched roots or
compiler reason; duplicate resolutions retain their candidate set.

`Tua: Why Is This Module Included?` opens the same panel focused on the active
file. The Why included tab shows one deterministic shortest path from a module
with no resolved importer to the selected file. Pure dependency cycles can have
no root path, which the inspector reports explicitly. Each step opens the exact
source import. The request runs as background project inspection and yields to
typing and interactive language features.

## Asset Explorer

The Explorer's Tua Assets view requests one compiler-owned project inventory
after the language server starts. It refreshes on source saves, asset file
events, explicit refreshes, and when the view becomes visible; it does not scan
or request inventory on each keystroke. Pending inventory work yields to typing
and other interactive language requests.

Assets are grouped by type, then Used or Unused. Expand a used asset to open its
exact static references, use Find Asset References for VS Code's references
view, or rename it from the item context menu. Rename uses a workspace file edit
so `workspace/willRenameFiles` can update every compiler-identified helper or
native LOVE path in the same operation. Unused means no static Tua/Lua loader
literal was indexed; dynamic loading can still use the file.

Hovering a resolved static asset literal offers Preview Asset. The hover action
uses the same language-server inventory as the Explorer and opens the resolved
file through VS Code's normal preview flow.

## Profiling

`Tua: Save Cache Stats` writes the current `tua/cacheStats` response as JSON
under VS Code global extension storage, in a `profiles` directory outside the
workspace source tree by default. The saved path is printed in the Tua output
channel and shown in the completion message.

`Tua: Reset Completion Learning Stats` clears local project-adaptive completion
ranking data for the current workspace. Tua stores only symbol-frequency counts
and structural hashes under `.tua/completion-stats.json`; it does not send source
content or ranking data off-device.

`Tua: Save CPU Profile` and `Tua: Save Heap Summary` request a server-provided
profile artifact directory outside the workspace. Current Tua servers may not
support those artifact kinds; unsupported or disabled profiling reports a
warning instead of failing the extension host. The extension intentionally does
not expose runtime operations that Tua does not have, such as a TypeScript/Go
garbage-collection command.

## Support

`Tua: Copy Debug Info` copies extension version, VS Code version, platform,
workspace trust, server state, resolved server path, server version, workspace
directory, and recent startup failure text when available. It does not read or
copy source file contents.

`Tua: Report Issue` opens the project issue URL with the same diagnostic bundle
pre-filled for review before submission. The extension does not send telemetry
or submit external reports automatically.

## Proposed Feature Policy

Tua prefers standard LSP requests over TypeScript-specific custom protocol:

- Hover uses `textDocument/hover`; no separate verbose-hover command exists.
  The extension can add trusted command links to hover results, but references
  and implementations still call standard VS Code/LSP providers.
- Source navigation uses definition, type-definition, validated asset targets,
  and `tua:/builtins` virtual documents for generated builtin or LOVE stubs.
- Asset references find every indexed Tua/Lua loader literal that resolves to
  the same file.
- Asset inventory, casing checks, unused hints, and rename edits come from the
  language server; the Explorer does not parse source or resolve paths itself.
- Project-wide occurrences use references instead of automatic multi-document
  highlights.
- Auto insert is out of scope until Tua has a safe syntax trigger; snippets and
  standard completion are the preferred path for block helpers.
- Sprite animation previews are exposed as an explicit command gated by
  `tua.preview.enabled`; they request compiler-backed `spriteSheet({...})`
  metadata through `tua/inspect` and draw resolved local image frames in a
  sticky webview without executing project code. Hovering a sprite sheet
  binding also uses the inspector result to offer a trusted command link.
  Missing assets, invalid frame ranges, incomplete configuration, and duplicate
  animation names use the same diagnostics shown by the language server.
- `Tua: Show Signal Graph` requests compiler-validated SignalBus calls from the
  current project snapshot. The webview connects bus-aware events to files,
  keeps same-named events on independent buses separate, distinguishes
  declarations, listeners, emits, cleanup, and query calls, filters by event,
  bus, file, listener, and emitter, shows conflicting payload schemas directly,
  and can create typed listeners or `declare(...)` schemas from compiler
  metadata. It opens exact source locations without parsing Tua source in the
  extension. Standard hover, definition, references, and rename requests work
  directly on event strings.
- Module dependency visualization requests `tua/moduleGraph`. The compiler owns
  module identity, resolution states, cycles, reverse relationships, exports,
  and inclusion paths; the extension only filters, lays out, and navigates that
  structured result.
- `Tua: Show ECS Explorer` requests `tua/ecsGraph`. It lists compiler-indexed
  components, systems, and queries, shows conservative callback read/write
  access, navigates every resolved component/query relationship, and presents
  allocation-heavy patterns as insights rather than diagnostics. Imported
  component identities use normal compiler definition resolution.
- The optional external API bridge is design-only. The extension does not start
  a bridge session today.
