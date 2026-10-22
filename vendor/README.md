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
- `runtime/tuasprite.lua` is Tua's small, replaceable controller over anim8.
  It provides independent named playback, manual seeking, marker events, and
  registration-aware drawing. It follows the same require-driven copy and
  project-override rules.
- `runtime/tuasignal.lua`, `runtime/tuaecs.lua`, and
  `runtime/tuaobjectpool.lua` are Tua's bundled signal, ECS, and object-pool
  runtimes. They follow the same require-driven copy and project-override
  rules.
- `runtime/tuastatemachine.lua` is the lua-state-machine-compatible runtime
  used by `stateMachine({...})`. It is adapted from Kyle Conroy's MIT-licensed
  lua-state-machine implementation and retains the upstream license header.
- `runtime/tuastore.lua` is Tua's explicit typed runtime-state store used by
  `store({...})`. It has no LOVE or filesystem dependency.
- `runtime/baton.lua` is Andrew Minnich's MIT-licensed Baton v1.0.2 runtime used
  by `input({...})`. Tua retains Baton's API and only adds static typing,
  completion, validation, and require-driven build integration.

- `runtime/tuatween.lua` is rxi's MIT-licensed flux 0.1.5 (commit
  `bb330231b87eabf84fbd68322f13a6320db30a41`) with LuaDoc types and a typed
  export table added; its runtime code is otherwise unchanged. It backs the
  built-in `tween` module, which Tua types from these annotations.
- `runtime/tuacamera.lua` is Tua's MIT-licensed 2D camera behind the built-in
  `Camera` module, typed from its own annotations.
- `tests/` holds plain-Lua runtime tests, run by `make runtime-test`.

Vendored files should keep their upstream license headers intact.
