---
icon: lucide/wand-sparkles
---

# Formatter

The formatter owns presentation. Its role is comparable to Prettier: given
valid Tua source and `[format]` settings, it produces one deterministic layout.
It does not decide whether the program is correct and it does not publish lint
diagnostics.

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
stays aligned with the declaration. Continued control-flow conditions are also
indented once, whether they use enclosing parentheses or leading `and`/`or`
operators; the body begins one level below the completed header.

```toml title="tua.toml"
[format]
indent_width = 2
line_width = 100
quote_style = "preserve"
final_newline = true
```

`quote_style` accepts `preserve`, `double`, or `single`. Forced styles rewrite
only complete short-string literals and update delimiter escapes without
changing the string value. Long-bracket strings, multiline literal bodies,
and comments are always preserved.

Formatting is never reported as `TL4xxx` lint output. If CI should enforce the
canonical layout, run `tua fmt --check` as its own step.

