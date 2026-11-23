# Headless dialogue example

From this directory, use the feature-branch compiler:

```sh
/path/to/tua build
cd build
lua main.lua
```

The explicit `require("tuadialogue")` carries a checked constructor/method contract
and includes the runtime only when used. No LÖVE host is needed. Replace `print`
with a renderer consuming owned snapshots. The snapshot node is optional; guard it before reading. Present nodes always have
`text` and `options`, using an empty string/array when not applicable. `start()` selects the
entry node; `start("label")` selects a named node. Choices use stable IDs.

This slice uses structured documents. `.tdialogue` files, localization, reveal,
temporary-message timing, async commands and editor syntax are still pending.
