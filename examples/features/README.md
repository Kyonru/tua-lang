# features

Buildable feature tour for tua language and editor behavior.

```sh
cargo run -p tua-cli -- check
cargo run -p tua-cli -- build
```

From the repository root:

```sh
make check-example EXAMPLE=examples/features
make build-features
```

## Files

- `src/types.tua` covers primitive types, `any`, aliases, table shapes,
  optional fields, unions, literal string unions, erased enum sugar, and
  annotated locals.
- `src/functions.tua` covers annotated function parameters and returns,
  inferred return hovers, call checking, and table-field mutation.
- `src/compound.tua` covers `+=`, `-=`, `*=`, `/=`, and `%=` lowering for
  locals and field chains.
- `src/inference.tua` covers local type inlay hints, inferred function return
  hovers, parameter-name hints, field-chain inference from annotated locals,
  array/index inference, numeric and generic loop variable inference, and the
  permissive `any` boundary.
- `src/modules.tua` covers normal Lua `require(...)` usage, plain Lua module
  interop, and cross-file go to definition for required module exports.
- `src/require_graph.tua` covers snapshot-indexed require edges and export
  lookup for `.tua` and `.lua` modules.
- `src/scopes.tua` covers lexical scope maps, shadowed locals, and module
  aliases as scoped symbols.
- `src/project_types.tua` covers the project-aware type database: aliases
  resolved through `require` aliases and LuaDoc-provided types from `.lua`
  files, including LuaDoc overload and typed-vararg examples.
- `src/lua_cheatsheet.tua` is an extensive Lua-in-Tua cheatsheet based on the
  referenced Lua cheatsheet. It covers comments, primitives, tables, operators,
  shorthand function calls, lookups, control flow, functions, closures,
  varargs, modules, metatables, prototype inheritance, typed iterator hooks,
  OOP-style methods, error handling, standard libraries, coroutines,
  environments, debug/GC APIs, LuaJIT boundaries, and LÖVE helper surfaces,
  with comments describing how Tua should interpret each feature.
- `src/formatting_lints.tua` demonstrates the clean baseline expected by the
  opt-in `formatting_lints` diagnostic profile.
- `src/luacats_emit.tua` demonstrates the optional LuaCATS emission mode for
  aliases, annotated locals, function signatures, and stable module exports.
- `src/luals_provider.lua` and `src/luals_annotations.tua` demonstrate the
  expanded LuaLS/LuaCATS annotation subset: literal unions, optional shorthand,
  arrays, tuples, dictionaries, function types, and shallow metadata tags.
- `src/inline_ignores.tua` demonstrates opt-in line-targeted diagnostic
  suppression comments such as `tua-ignore-next-line` and `tua-ignore-line`.
- `src/luahelper_lints.tua` demonstrates opt-in LuaHelper parity diagnostics
  for assignment/local declaration arity, unused assignments, binary operator
  type mismatches, and local functions that are referenced but never directly
  called.
- `src/luajit.tua` demonstrates opt-in LuaJIT guidance comments for variable
  type drift, table-shape drift, hot-loop dynamic writes, and the `any` escape
  hatch. The feature is disabled by default, so the example remains buildable
  without warnings.
- `src/ecs.tua` demonstrates the opt-in tiny-ecs-shaped ECS domain support:
  component schemas, typed world/entity/system APIs, component value
  diagnostics, query diagnostics, and normal Lua adapter interop.
- `src/tiny_ecs_adapter.lua` is a tiny plain-Lua runtime adapter used by
  `src/ecs.tua`; real projects can bind `ecs` to tiny-ecs or a compatible
  module without changing Tua's checker model.
- `src/lua_doc_types.lua` is a plain Lua type provider used by
  `src/project_types.tua`; it demonstrates `---@class`, `---@field`,
  literal-union aliases, `---@overload`, `---@vararg`, and the LuaHelper
  `identifier` keyword.
- `src/navigation.tua` is a compact same-file sample for go to definition,
  references, rename, document symbols, inlay hints, and semantic highlighting.
- `src/assets.tua` covers typed LÖVE asset loader helpers such as
  `image "assets/player.png"`, `sound "assets/jump.wav"`,
  `music "assets/theme.ogg"`, `font("assets/ui.ttf", 18)`, `shader`,
  `imageData`, `canvas`, `quad`, `spriteBatch`, and `particles`, plus LOVE
  object method completion/signature behavior such as preferring
  `player:setFilter()` over `player.setFilter()`. Hover asset strings to see
  metadata, and edit inside the strings to try helper-specific asset-path
  completion. LOVE code actions can also rewrite obvious raw constructors into
  helper calls.
- `src/vectors.tua` covers the built-in LOVE-friendly `Vec2` type, the
  `vec2(x, y)` constructor, `x`/`y` field completion and hover, conservative
  BrineVector properties/operators, and bundled BrineVector Lua emission.
- `src/assets/` contains placeholder files used by static asset validation,
  asset-path completion, and asset metadata hovers.
- `src/main.tua` ties the feature modules together while keeping `require`
  calls unchanged in emitted Lua.
- `src/legacy.lua` is intentionally plain Lua. Builds copy it unchanged into
  `build/`, while the LSP can still infer simple returned module exports from
  it.
