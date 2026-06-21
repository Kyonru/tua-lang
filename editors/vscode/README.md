# tua VS Code Extension

Minimal VS Code language support for tua source files.

## Features

- Registers the `tua` language id.
- Associates `*.tua` files with the tua language mode.
- Adds TextMate grammar coverage for Lua keywords, comments, strings, numbers,
  operators, type aliases, and type annotations.
- Starts `tua lsp` over stdio and surfaces the language server's diagnostics,
  hover, completion, navigation, rename, inlay hint, semantic token, and symbol
  features.

## Configuration

- `tua.server.path`: path to the `tua` executable. Defaults to `tua` on `PATH`.

If the executable is missing, activation shows a warning and links directly to
the setting.
