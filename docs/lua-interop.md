---
icon: lucide/repeat-2
---

# Lua Interop

Tua is designed for mixed Lua and Tua projects. Both languages use normal Lua
modules at runtime, `.lua` files can live beside `.tua` files, and generated
output remains ordinary Lua.

!!! info "One project, one Lua module system"
    `require("module")` is the runtime module mechanism for both Lua and Tua.
    Tua's static `import(...)` helper only adds compile-time path rules and
    editor guarantees; it still emits a normal `require(...)` call.

## Choose A Module Form

| Form | Use it for | Runtime output |
| --- | --- | --- |
| `require("module")` | Lua packages, mixed Lua/Tua modules, and dynamic module names | Preserved as `require("module")` |
| `import("shared")` | Statically known Tua project modules | Lowered to `require("shared")` |
| `import("./player")` | A child Tua module relative to the current source file | Lowered to the configured project module path |

Static imports reject parent traversal such as `import("../shared")`. Use a
project module name such as `import("shared")` or `import("engine.shared")`
instead. Use `require(...)` when a module comes from an external package or its
name is computed at runtime.

A typical mixed project needs no special directory:

```text
src/
|-- main.tua
|-- player.tua
`-- legacy.lua
```

During `tua build`, Tua sources emit as Lua and plain Lua sources are copied
unchanged when the source and output roots differ. Both are then loaded through
the same runtime module paths under `build/`.

## Use A Lua Module From Tua

Tua can infer simple Lua module exports, but LuaDoc annotations provide stable
types and documentation at the boundary.

```lua title="src/legacy.lua"
---@alias EnemyKind "friendly" | "hostile"

---@class Enemy
---@field name string display name
---@field hp number remaining health
---@field kind EnemyKind

---@param name string
---@return Enemy
local function makeEnemy(name)
  return {
    name = name,
    hp = 20,
    kind = "hostile",
  }
end

return {
  makeEnemy = makeEnemy,
}
```

The returned table becomes the module export surface:

```tua title="src/main.tua"
local Legacy = require("legacy")

local enemy = Legacy.makeEnemy("slime")
enemy.hp -= 5
```

The editor can complete `Legacy.makeEnemy`, show its signature and
documentation, infer `enemy` as `Enemy`, and complete `enemy.name`, `enemy.hp`,
and `enemy.kind`.

!!! tip "Keep exports statically visible"
    Return a table literal, or return a local that contains a clear table
    literal, when you want Tua to understand a Lua module's fields. Runtime
    mutation, computed export names, and complex metatable construction remain
    dynamic and may produce `any`.

## Import Tua Modules Statically

Use `import(...)` when both sides are Tua project modules and you want the
specifier checked as a static project path:

```tua
local Player = import("./player")
local Shared = import("shared")
```

Static imports participate in the same project module graph as
`require("...")`. They support module completion, diagnostics, navigation,
rename-aware path updates, dependency-cycle inspection, and exported type
inference before lowering to Lua.

In VS Code:

- **Tua: Show Module Dependency Graph** displays resolved and unresolved
  imports, cycles, reverse dependencies, and inferred exports.
- **Tua: Why Is This Module Included?** shows the shortest static path from a
  project root candidate to the active file.

Every graph step opens the original `require` or `import` range. The extension
reads the compiler's module graph instead of parsing source independently.

## Use A Tua Module From Lua

!!! warning "Lua loads emitted modules"
    Lua cannot require `.tua` source directly. Build the project first, run Lua
    from the configured output directory, and require the emitted module path.

=== "Tua"

    ```tua title="src/player.tua"
    type Player = {
      name: string,
      hp: number,
    }

    local function makePlayer(name: string): Player
      return {
        name = name,
        hp = 100,
      }
    end

    return {
      makePlayer = makePlayer,
    }
    ```

=== "Generated Lua"

    ```lua title="build/player.lua"
    local function makePlayer(name)
      return {
        name = name,
        hp = 100,
      }
    end

    return {
      makePlayer = makePlayer,
    }
    ```

Lua code running from `build/` can load the module normally:

```lua
local Player = require("player")
local player = Player.makePlayer("Ada")
```

### Preserve Types For Lua Tools

Tua erases annotations by default. If generated Lua will be consumed in an
editor that understands LuaCATS/LuaLS comments, enable annotation-preserving
emission:

```toml title="tua.toml"
[emit]
annotation_mode = "luacats"
```

The runtime code stays plain Lua, while useful aliases, locals, function
signatures, and stable module exports receive generated comments:

```lua
---@alias Player { name: string, hp: number }

---@param name string
---@return Player
local function makePlayer(name)
  return {
    name = name,
    hp = 100,
  }
end

---@class PlayerExports
---@field makePlayer fun(name: string): Player
return {
  makePlayer = makePlayer,
}
```

LuaCATS mode affects generated documentation comments only. It does not add a
runtime dependency or change Lua behavior.

## LuaDoc Support

Tua parses LuaHelper/LuaLS-style annotations in both `.lua` and `.tua` files.
The most useful annotations establish types at module and callback boundaries.

### Type-Bearing Tags

| Tag | Current use |
| --- | --- |
| `---@class` | Declares a named table or object shape. |
| `---@field` | Adds a typed field, optional field, visibility metadata, and documentation to a class. |
| `---@alias` | Declares a reusable type alias or literal union. |
| `---@type` | Types an attached local or value where the relationship is statically clear. |
| `---@param` | Types and documents a function parameter. |
| `---@return` | Types and documents one or more function returns. |
| `---@overload` | Adds another callable signature to a known function. |
| `---@vararg` | Types extra `...` arguments. |

These annotations can contribute to checking, module export inference, hover,
completion, signature help, semantic highlighting, and parameter
documentation.

### Supported Type Forms

| Shape | Example |
| --- | --- |
| Primitive or alias | `number`, `string`, `boolean`, `nil`, `Enemy` |
| Union or literal choice | `number \| nil`, `"static" \| "stream"` |
| Optional shorthand | `number?` |
| Array | `Image[]` |
| Tuple | `[number, string]` |
| Dictionary | `{ [string]: Source }` |
| LuaDoc key-value table | `table<string, Image>` |
| Function | `fun(path: string): Image` |
| Receiver identity | `Self` inside supported class and colon-method annotations |

The LuaHelper `identifier` type is treated as `string`. LuaDoc
`fun(value: number, ...: string): string` and native Tua
`(value: number, ...string) -> string` lower to the same semantic function
type.

### Overloads And Typed Varargs

Use overloads for a small set of distinct call shapes:

```lua
---@overload fun(path: string): Image
---@overload fun(path: string, mode: "stream"): Source
local function loadAsset(path, mode)
  if mode == "stream" then
    return love.audio.newSource(path, mode)
  end

  return love.graphics.newImage(path)
end
```

Signature help selects among known overloads, and call diagnostics use a
matching overload when Tua can prove one applies.

Use `---@vararg` to type flexible Lua functions:

```lua
---@param message string
---@vararg identifier
local function logLabels(message, ...)
  print(message, ...)
end
```

### Parsed Metadata

Tua also recognizes `---@generic`, `---@version`, `---@enum`, `---@meta`,
`---@module`, `---@diagnostic`, `---@cast`, `---@as`, `---@deprecated`, and
`---@nodiscard`. Some of these tags are preserved with source spans and
documentation but do not currently change type checking.

!!! note "Recognition does not imply full LuaLS semantics"
    Tua implements the practical annotation subset needed for typed Lua
    boundaries. `---@generic` is metadata only because Tua has no generic
    solver, and advanced LuaLS annotation behavior degrades conservatively
    instead of making valid Lua fail.

Malformed supported annotations can report diagnostics when enabled:

```toml title="tua.toml"
[diagnostics]
doc_annotations = true
```

## Builtins, LOVE, And External APIs

Tua includes a curated Lua catalog and a LOVE 11.x catalog generated from a
pinned `love2d-community/love-api` revision. These catalogs provide completion,
hover, signature help, literal choices, callback validation, object methods,
and object inheritance without requiring project stubs.

Values returned from known LOVE constructors retain object types:

```tua
local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")

player:setFilter("nearest")
jump:play()
```

Use colon syntax for object methods. When Tua proves a LOVE receiver, a
dot-style call such as `player.setFilter("nearest")` reports `TL4018`, and
completion can rewrite the separator.

For libraries outside the built-in catalogs, keep their Lua modules in the
project and annotate public boundaries, or provide simple annotated Lua stub
modules that return the same export shape as the runtime library.

??? note "Maintaining the generated LOVE catalog"
    The catalog revision is pinned in `scripts/love-api-revision.txt`.
    Maintainers can regenerate or verify all generated surfaces with:

    ```bash
    make generate-love-api LOVE_API_ROOT=/path/to/love-api
    make love-api-drift-check LOVE_API_ROOT=/path/to/love-api
    ```

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| A required Lua module is `any` | Return a statically visible table and annotate its public functions or values. |
| A module field does not complete | Confirm the field is present in the returned export table rather than added dynamically later. |
| A module path is unresolved | Check `source`, file location, spelling, and the configured require-path separator. Use `require` for external packages. |
| A parent relative import is rejected | Replace `import("../shared")` with a project module name such as `import("shared")`. |
| Lua cannot load a Tua module | Run `tua build` and execute from the output directory so Lua sees the emitted `.lua` file. |
| Generated Lua has no type comments | Set `[emit].annotation_mode = "luacats"`; the default is `"off"`. |
| A malformed annotation stays quiet | Enable `[diagnostics].doc_annotations`. |

!!! warning "Static analysis never executes project Lua"
    Tua does not run `require(...)`, callbacks, metatable setup, or library code
    to discover types. Computed exports and runtime-only identities remain
    `any` unless a stable annotation or statically visible shape describes
    them.

## Practical Guidance

- Keep module return tables explicit.
- Annotate public parameters, returns, callbacks, and exported shapes rather
  than every internal local.
- Prefer static `import(...)` for known Tua project modules and `require(...)`
  for Lua or dynamic package boundaries.
- Enable LuaCATS emission when generated Lua is a public or editor-consumed
  artifact.
- Use `any` deliberately when a value is genuinely dynamic.
