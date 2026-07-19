# Vendored Runtime Files

This directory contains third-party Lua runtime files that Tua may copy into
generated project output.

- `runtime/brinevector.lua` is the bundled BrineVector runtime used by
  `vec2(x, y)` emission. `tua build` copies it to `build/brinevector.lua` only
  when generated Lua requires it, and only when the project did not already
  provide its own `brinevector.lua`.
- `runtime/anim8.lua` is the bundled anim8 runtime used by experimental sprite
  helper emission. `tua build` copies it to `build/anim8.lua` only when generated
  Lua requires it, and only when the project did not already provide its own
  `anim8.lua`.
- `runtime/tuasignal.lua`, `runtime/tuaecs.lua`, and
  `runtime/tuaobjectpool.lua` are Tua's bundled signal, ECS, and object-pool
  runtimes. They follow the same require-driven copy and project-override
  rules.
- `runtime/tuastatemachine.lua` is the lua-state-machine-compatible runtime
  used by `stateMachine({...})`. It is adapted from Kyle Conroy's MIT-licensed
  lua-state-machine implementation and retains the upstream license header.

Vendored files should keep their upstream license headers intact.
