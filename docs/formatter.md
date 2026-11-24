---
icon: lucide/wand-sparkles
---

# Formatter

Use the formatter to keep indentation, spacing, quotes, and line endings
consistent. Its default layout separates crowded statements and expands multiline
tables into one field per line. It formats parseable Tua source using the
project's `[format]` settings. Type checking and linting are separate commands.

## Format In VS Code

Run **Format Document**, or enable formatting on save in `.vscode/settings.json`:

```json
{
  "[tua]": {
    "editor.defaultFormatter": "SolenodonteLabs.tua",
    "editor.formatOnSave": true
  }
}
```

The extension also supports range and on-type formatting. If formatting does
nothing, check the file's language mode, the selected formatter, and syntax
errors in Problems. See [Troubleshooting](troubleshooting.md#formatting-does-not-run).

## Format From The Terminal

```bash
tua fmt
tua fmt --check
tua fmt src/main.tua tests/collision.lua
tua fmt --check tests/collision.lua
tua fmt --stdin-filepath tests/collision.lua < tests/collision.lua
```

`tua fmt` rewrites project `.tua` and `.tdialogue` files, including the configured typed test root.
Pass explicit paths to format selected `.tua`, `.lua`, `.tdialogue`, or configured-extension
files with the current project's settings, including files outside the source
root. Paths must stay within the project; directories, symlinks, and non-source
files are rejected. Duplicate paths are processed once. Use `--` before a path
whose name starts with a dash. Without paths, `.lua` assets remain excluded from
a normal Tua project scan.

`--stdin-filepath PATH` reads source from stdin and writes only formatted source
to stdout, with no summary or added newline. The path selects the source filename
for diagnostics and must satisfy the same project/extension rules; it need not
exist and is never written. Use the current project's `[format]` settings (or
defaults outside a configured project). Stdin and explicit files cannot be mixed.
With `--check`, stdin mode emits no source and succeeds only when already
formatted. Invalid syntax, invalid UTF-8, and input above the source byte limit
(8 MiB) fail. This supports editor buffers and scripts without temporary projects.
`tua fmt --check` is the non-mutating CI form: it exits unsuccessfully and lists files whose formatted output differs
from the source. The language server exposes the same formatter for document,
range, and on-type formatting for Lua/Tua. Dialogue uses a [dedicated full-document
formatter](dialogue.md#format-a-script) that preserves prose and metadata; dialogue
range and on-type formatting remain disabled.

## Layout Rules

Core token spacing has one fixed style; indentation, wrapping, quotes, and
line endings use the project settings. `final_newline` controls the final
line terminator.

Spacing follows one style: binary operators and `=` are surrounded by spaces,
unary `-` and `#` attach to their operand, one-line tables are padded as
`{ 1, 2 }` while an empty table stays `{}`, and a trailing comment is
separated from code by one space.

A table field starting with unary minus uses
normal field indentation; a subtraction continued from the previous line uses
continuation indentation. Grouped expressions keep a space after `and`, `or`,
and `not`, as in `ready and (not stopped)`, while function calls stay compact
as `run(value)`.

Consecutive statements on one line are separated onto their own lines. An inline
block containing several statements is expanded with its body and closing keyword
on separate lines. Optional separators between those statements are removed; a
semicolon before a parenthesized call is retained where it disambiguates Lua call
syntax. Long inline bodies expand before argument wrapping. Small single-action
guards and simple callbacks can remain compact.

Multiline runtime tables have one field per line and a separate closing brace.
Small nested tables may stay compact. Expanding statements inside a table callback
also expands the enclosing table in the same pass. These are shared defaults in
the CLI and language server, not project-specific lint rules or an opt-in preset.

For example, crowded setup and a multiline record become:

```lua
local count = 0
local enabled = true
local options = {
  count = count,
  enabled = enabled,
  offset = { x = 1, y = 2 },
}
if enabled then
  count += 1
  print(count)
end
```

String and comment contents are never reformatted. This includes nonblank
lines inside long-bracket strings, block comments, and escaped multiline short
strings, even when their text resembles Tua code.

Parentheses opened on the same line, as in `print(lookup(`, add one
indentation level, and the line that closes them returns to the opening line's
indentation.

Multiline function parameters are indented once while the closing parenthesis
stays aligned with the declaration. Multiline calls in the body indent their
arguments one level beyond the call, including inside callbacks and functions
with multiline parameter lists. Continued control-flow conditions are also
indented once, whether they use enclosing parentheses or leading `and`/`or`
operators; the body begins one level below the completed header.

## Configure The Layout

```toml title="tua.toml"
[format]
indent_style = "spaces"
indent_width = 2
line_width = 100
quote_style = "preserve"
line_ending = "lf"
final_newline = true
```

`indent_style` accepts `spaces` or `tabs`. `indent_width` controls spaces per
level and is ignored for tab indentation. `indent_width` and `line_width` must
both be at least 1. Invalid TOML values
are reported as configuration errors; invalid editor overrides are ignored in
favor of the project/default setting.

`quote_style` accepts `preserve`, `double`, or `single`. Forced styles rewrite
only complete short-string literals and update delimiter escapes without
changing the string value. Long-bracket strings, multiline literal bodies,
and comments are always preserved.

`line_ending` accepts `lf`, `crlf`, or `preserve`. Preserve follows the source
file's existing LF/CRLF convention; mixed files are normalized to CRLF when
any CRLF line is present. `final_newline` independently controls whether the
formatted document ends with a line terminator.

Formatting is never reported as `TL4xxx` lint output. If CI should enforce the
canonical layout, run `tua fmt --check` as its own step.

## Current scope

`line_width` is a preferred wrapping width, not a hard column limit. When an
outer table cannot be wrapped safely, the formatter leaves it intact rather
than expanding an inner call on its own. The
formatter currently expands comma-separated call arguments and table fields;
it does not yet reflow long binary expressions, method chains, comments,
strings, or single-argument calls. Short single-statement blocks, such as
`function(a, b) return a end`, stay compact. Multiple statements are split using
parsed block boundaries; comments and lowered constructs whose original boundaries
cannot be mapped safely retain their existing layout.

Formatting requires parseable source. A leading UTF-8 byte-order mark is
accepted, as LuaJIT accepts it, and removed from formatted output.
Whole-document formatting normalizes the configured line-ending convention,
while range and on-type formatting limit their edits to the requested source
area. Project `[format]` settings are canonical and take precedence over an
editor client's generic tab-size and insert-spaces preferences.

Tua does not currently provide formatter-disable directives, configurable
trailing commas, or path/stdin formatting modes. Type-annotation punctuation
also remains source-preserving where changing it could be confused with Lua's
method-call colon. These forms currently retain their source layout.

