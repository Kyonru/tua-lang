# tua VS Code Extension

Minimal VS Code language support for tua source files.

## Features

- Registers the `tua` language id.
- Associates `*.tua` files with the tua language mode.
- Uses the tua logo SVG for `.tua` file icons and a generated PNG copy for the
  packaged extension icon.
- Adds TextMate grammar coverage for Lua keywords, comments, strings, numbers,
  operators, type aliases, and type annotations.
- Starts `tua lsp` over stdio and surfaces the language server's diagnostics,
  hover, completion, signature help, document highlights, navigation, rename,
  inlay hint, semantic token, and symbol features.

## Configuration

- `tua.server.path`: path to the `tua` executable. Defaults to `tua` on `PATH`.

VSIX packages built with `make extension-package` include the current platform's
`tua` server binary and use it by default.

For extension-host development, `TUA_SERVER_PATH` overrides the setting.
Use the workspace `Run Extension (Features Example)` launch profile to open
`examples/features` with `target/debug/tua`. Use
`Run Extension (Features Example, Bundled Server)` to compare against a staged
bundled server. `Run Extension (Debug Server)` opens `examples/love-full` with
the debug server.

If the executable is missing, activation shows a warning and links directly to
the setting.
