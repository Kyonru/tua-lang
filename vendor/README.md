# Vendored Runtime Files

This directory contains third-party Lua runtime files that Tua may copy into
generated project output.

- `runtime/brinevector.lua` is the bundled BrineVector runtime used by
  `vec2(x, y)` emission. `tua build` copies it to `build/brinevector.lua` only
  when generated Lua requires it, and only when the project did not already
  provide its own `brinevector.lua`.

Vendored files should keep their upstream license headers intact.
