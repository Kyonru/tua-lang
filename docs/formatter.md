---
icon: lucide/wand-sparkles
---

# Formatter

The formatter owns presentation. Its role is comparable to Prettier: given
valid Tua source and `[format]` settings, it produces one deterministic layout.
It does not decide whether the program is correct and it does not publish lint
diagnostics.

Core token spacing is deliberately not configurable. Assignments, arithmetic,
comparisons, concatenation, and function-type arrows use one canonical spaced
form; unary signs, scientific-notation signs, field access, and varargs remain
compact.

```bash
tua fmt
tua fmt --check
```

`tua fmt` rewrites project `.tua` files. `tua fmt --check` is the non-mutating
CI form: it exits unsuccessfully and lists files whose formatted output differs
from the source. The language server exposes the same formatter for document,
range, and on-type formatting.

The formatter controls indentation, line wrapping, quote preservation, final
newlines, spacing, and blank-line normalization. Configure it independently:

String and comment contents are never reformatted. This includes nonblank
lines inside long-bracket strings, block comments, and escaped multiline short
strings, even when their text resembles Tua code.

Multiline function parameters are indented once while the closing parenthesis
stays aligned with the declaration. Multiline calls in the body indent their
arguments one level beyond the call, including inside callbacks and functions
with multiline parameter lists. Continued control-flow conditions are also
indented once, whether they use enclosing parentheses or leading `and`/`or`
operators; the body begins one level below the completed header.

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

`line_width` is a preferred wrapping width, not a hard column limit. The
formatter currently expands comma-separated call arguments and table fields;
it does not yet reflow long binary expressions, method chains, comments,
strings, or single-argument calls. It also leaves a block that opens and closes
on one line, such as `function(a, b) return a end`, intact rather than splitting
its header from its `end`. Syntax-aware reflow for those forms belongs in a
future document-layout engine rather than more line-level heuristics.

Formatting requires parseable source. A leading UTF-8 byte-order mark is
accepted, as LuaJIT accepts it, and removed from formatted output.
Whole-document formatting normalizes the configured line-ending convention,
while range and on-type formatting limit their edits to the requested source
area. Project `[format]` settings are canonical and take precedence over an
editor client's generic tab-size and insert-spaces preferences.

Tua does not currently provide formatter-disable directives, configurable
trailing commas, or path/stdin formatting modes. Type-annotation punctuation
also remains source-preserving where changing it could be confused with Lua's
method-call colon. These are deliberate boundaries of the current line-based
formatter, not claims of full Prettier-style reflow.

