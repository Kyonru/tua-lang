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
  copying or saving cache statistics, probing server profile artifacts,
  previewing `spriteSheet` animations, running source actions, reporting an
  issue, and opening Tua settings.
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

For extension-host development, `TUA_SERVER_PATH` overrides the setting.
Use the workspace `Run Extension (Features Example)` launch profile to open
`examples/features` with `target/debug/tua`. Use
`Run Extension (Features Example, Bundled Server)` to compare against a staged
bundled server. `Run Extension (Debug Server)` opens `examples/love-full` with
the debug server.

`Run Extension (All Examples)` enables `TUA_DEBUG_LSP=1`, which writes
extension typing/suggestion trace lines and server completion trace messages to
the Tua output channel.

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
- `Tua: Save CPU Profile`
- `Tua: Save Heap Summary`
- `Tua: Preview Sprite Animations`
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
- Source navigation uses definition, type-definition, and `tua:/builtins`
  virtual documents for generated builtin or LOVE stubs.
- Project-wide occurrences use references instead of automatic multi-document
  highlights.
- Auto insert is out of scope until Tua has a safe syntax trigger; snippets and
  standard completion are the preferred path for block helpers.
- Sprite animation previews are exposed as an explicit command gated by
  `tua.preview.enabled`; they read `spriteSheet({...})` metadata from the active
  document and draw local image frames in a sticky webview preview without
  executing project code. Hovering a parsed sprite sheet binding also offers a
  trusted command link to open the same preview.
- The optional external API bridge is design-only. The extension does not start
  a bridge session today.
