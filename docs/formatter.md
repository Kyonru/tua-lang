---
icon: lucide/wand-sparkles
---

# Formatter

Tua includes a Rust-native, StyLua-based formatter with support for authored Tua
syntax. The CLI and language server share the same implementation and project
settings. You do not need to install StyLua separately.

The defaults use **two spaces**, a **preferred width of 100**, and
**double-preferred quotes**. Formatting preserves type annotations, defaults,
classes, protocols, enums, type-only imports and `is` tests. It never formats
lowered Lua and reconstructs your declarations afterward.

## Format In VS Code

Run **Format Document**, or enable formatting on save:

```json title=".vscode/settings.json"
{
  "[tua]": {
    "editor.defaultFormatter": "SolenodonteLabs.tua",
    "editor.formatOnSave": true
  }
}
```

Range and on-type formatting use the same style. Their edits stay within the
requested source area. An incomplete file or a selection that does not enclose
formattable syntax may produce no edit; finish the statement or select a larger
region. Project settings take precedence over the editor's generic tab-size and
insert-spaces preferences.

If formatting does nothing, check the language mode, selected formatter and
syntax errors in Problems. See [Troubleshooting](troubleshooting.md#formatting-does-not-run).

## Format From The Terminal

```bash
tua fmt
tua fmt --check
tua fmt src/main.tua tests/collision.lua
tua fmt --stdin-filepath tests/collision.lua < tests/collision.lua
```

Without paths, `tua fmt` rewrites project `.tua` and `.tdialogue` files, including
the configured typed test root. Plain `.lua` assets are excluded from that scan;
pass their paths explicitly to format them. Plain Lua formatting accepts LuaJIT
syntax, including integer suffixes, hexadecimal floats, `goto` and labels. These
forms remain subject to the stricter language rules in `.tua` files. If the
project configures `.lua` as its typed source extension, formatting respects
that classification and uses Tua syntax.

Explicit paths accept `.tua`, `.lua`, `.tdialogue` and configured-extension files,
including files outside the source root but within the project. Directories,
symlinks and non-source files are rejected. Duplicate paths are processed once.
Use `--` before a filename that starts with a dash.

`--check` writes nothing and exits unsuccessfully when formatting would change a
file. Use it as a separate CI step: formatting differences are not lint errors.

`--stdin-filepath PATH` reads stdin and writes only formatted source to stdout.
The path selects project settings and diagnostic names; it need not exist and is
never written. Stdin cannot be combined with explicit files. With `--check`, it
emits no source and checks whether stdin is already formatted. Invalid syntax,
invalid UTF-8 and input above the 8 MiB source limit fail.

## What Changes

The formatter separates statements, standardizes spaces around operators and
type annotations, and wraps calls, expressions and type declarations. Nonempty
function and control-flow bodies expand onto separate lines. Empty callbacks
can stay compact. Short tables can fit on one line; multiline tables use one
field per line and a separate closing brace.

For example:

```tua
local count: number = 0
local options = { enabled = true, offset = { x = 1, y = 2 } }

if options.enabled then
  count += 1
  print(count)
end
```

String values and comment contents are preserved. Quote delimiters and escapes
may change according to the selected quote policy. Long-string line endings
follow the configured newline convention without changing their Lua value.
Necessary parentheses and disambiguating statement separators are retained.

`line_width` is a preferred width, not a hard limit. Long identifiers, comments
and string contents are not split to fit. Existing call parentheses are retained;
the formatter does not turn `require "module"` into `require("module")` merely
for style.

## Configure The Layout

```toml title="tua.toml"
[format]
indent_style = "spaces"
indent_width = 2
line_width = 100
quote_style = "auto-double"
line_ending = "lf"
final_newline = true
```

`quote_style` accepts:

- `auto-double`: prefer double quotes, choosing single quotes when they avoid escapes.
- `preserve`: retain each short string's authored delimiter and escape spelling.
- `double` or `single`: force that delimiter while preserving the string value.

Earlier versions defaulted to `preserve`. Set it explicitly if you want to retain
that quote policy when adopting the new layout.

`indent_style` accepts `spaces` or `tabs`; widths must be at least 1.
`line_ending` accepts `lf`, `crlf` or `preserve`. Preserve chooses CRLF if the
source contains any CRLF line endings, otherwise LF. `final_newline` controls the
final terminator independently.

Use `[format]` in `tua.toml`, or the extension's `tua.format.*` overrides. A separate
`stylua.toml` is not read. See [configuration](configuration.md#format) for the
complete settings reference.

## Keep A Statement's Layout

A StyLua ignore comment preserves the following statement or declaration:

```tua
-- stylua: ignore
local lookup = { [0] = "off", [1] = "on" }
```

Use `-- stylua: ignore start` and `-- stylua: ignore end` around statements in the
same block to preserve a larger region. Ignored source must still parse.

## Validation And Other File Types

Formatting checks the authored AST and validates the result with the matching
Tua or LuaJIT parser.
Range and on-type edits are checked after narrowing them to the requested area;
if that partial edit would change the program, no edit is returned.
It does not require type checking to succeed or imported modules to be loaded.
An unsafe result is reported instead of being written. A leading UTF-8 byte-order
mark is accepted and removed.

Dialogue scripts use their own [prose-preserving formatter](dialogue.md#format-a-script),
not Lua layout. Dialogue range and on-type formatting remain disabled.
