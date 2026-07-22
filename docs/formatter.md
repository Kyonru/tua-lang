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

```toml title="tua.toml"
[format]
indent_width = 2
line_width = 100
quote_style = "preserve"
final_newline = true
```

Formatting is never reported as `TL4xxx` lint output. If CI should enforce the
canonical layout, run `tua fmt --check` as its own step.

