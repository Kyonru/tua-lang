# tua VS Code Extension

Minimal VS Code language support for tua source files.

## Features

- Registers the `tua` language id.
- Associates `*.tua` files with the tua language mode.
- Uses the tua logo SVG for `.tua` file icons and a generated PNG copy for the
  packaged extension icon.
- Adds TextMate grammar coverage for Lua keywords, comments, strings, numbers,
  operators, type aliases, and type annotations.
- Adds language configuration for comments, brackets, auto-closing pairs,
  region folding, word boundaries, and Lua/Tua indentation.
- Bundles instant snippets for Lua functions, typed Tua declarations, classes,
  loops, conditionals, and common LOVE callbacks.
- Starts `tua lsp` over stdio and surfaces the language server's diagnostics,
  hover, completion, signature help, document highlights, navigation, rename,
  inlay hint, semantic token, and symbol features.
- Contributes Tua editor defaults for zero-delay quick suggestions in code and
  strings.
- Adds Tua command-palette actions for restarting the language server, focusing
  the output channel, selecting a server binary, copying debug information,
  copying or saving cache statistics, resetting completion learning stats,
  probing server profile artifacts, previewing `spriteSheet` animations,
  visualizing SignalBus events across files, running source actions, reporting
  an issue, and opening Tua settings.
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
signature help, inlay hints, UTF-16 and untitled documents, file renames, and
sprite preview opening, reload, focus, missing assets, and hover command links.

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
- `Tua: Preview Sprite Animations`
- `Tua: Show Signal Graph`
- `Tua: Organize Imports`
- `Tua: Sort Imports`
- `Tua: Remove Unused Imports`
- `Tua: Fix All Safe Diagnostics`
- `Tua: Report Issue`

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
- Source navigation uses definition, type-definition, validated asset targets,
  and `tua:/builtins` virtual documents for generated builtin or LOVE stubs.
- Asset references find every indexed Tua/Lua loader literal that resolves to
  the same file.
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
  declarations, listeners, emits, cleanup, and query calls, and opens exact
  source locations without parsing Tua source in the extension. Standard hover,
  definition, references, and rename requests work directly on event strings.
- The optional external API bridge is design-only. The extension does not start
  a bridge session today.
