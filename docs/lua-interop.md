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

The Luau-capable parser used internally does not make Luau syntax part of Tua.
A post-parse dialect check reports `TL0004` for non-Tua constructs before
checking or emission. Generated `.lua` files then pass a final Lua 5.1 parse
before Tua returns an emission artifact; `TL2010` is the blocking safety
diagnostic if compiler lowering or emission violates that invariant. Plain
`.lua` files remain outside this transpilation step and are copied unchanged.

## Use A Lua Module From Tua

Tua can infer simple Lua module exports, but LuaDoc annotations provide stable
types and documentation at the boundary.

Inference follows the module's statically visible returned value rather than
requiring a separate declaration file. It understands primitive values, nested
tables, named and anonymous function returns, multiple returns, staged table
assignments, direct local aliases, conservative conditional fields, and the
common `setmetatable` constructor pattern:

```lua title="src/player.lua"
local Player = {}
Player.__index = Player

function Player.new(name)
  return setmetatable({
    name = name,
    x = 0,
    active = true,
  }, Player)
end

function Player:update(dt)
  self.x = self.x + 100 * dt
end

return Player
```

```tua title="src/main.tua"
local Player = require("player")
local player = Player.new("Ada")

player.x += 10        -- number
player.active = false -- boolean
player:update(0.016)  -- method retained from Player.__index
```

Lua modules may also return a function directly. Its inferred return values
flow through the require binding:

```lua title="src/status.lua"
return function(message)
  return message, true
end
```

```tua title="src/main.tua"
local status = require("status")
local message, ok = status("ready") -- any, boolean
```

These module surfaces are cached with the parsed Lua file. Importers are
invalidated only when the inferred public surface changes, so hovering or
completing a required module does not recursively analyze files.

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

Unannotated function parameters remain `any`: Tua does not guess contracts from
parameter names or a handful of call sites. Add LuaDoc when callers need a
checked parameter contract, documentation, overloads, or a stable name for a
recursive shape. Computed keys, data-dependent module assembly, and unknown
external calls also stay permissive.

### Large Generated Modules

Generated data can remain a checked `.tua` module without making the checker
infer one enormous structural union. Put `--!tua generated` within the first
eight lines and describe the data with LuaDoc:

```lua title="src/rooms.tua"
--!tua generated
-- Generated by tools/generate_rooms.mjs. Do not edit.

---@class RoomRecord
---@field name string
---@field minX number
---@field maxX number

---@type RoomRecord[]
local rooms = {
  { name = "Level1", minX = -320, maxX = 4096 },
  { name = "Level2", minX = -128, maxX = 6144 },
}

local Rooms = {}

---@return RoomRecord[]
function Rooms.all()
  return rooms
end

return Rooms
```

The directive is an explicit parser contract: executable source in the module
must remain Lua 5.1-compatible. Tua parses it directly with the Lua grammar,
keeps its Tua module identity, validates array entries individually against the
`---@type` schema, and emits ordinary Lua. Native Tua annotations and other
Tua-only syntax are intentionally rejected at this generated boundary.

Contextual validation keeps peak inferred type size proportional to one array
entry. A malformed entry still produces `TL1001`; table shapes that cannot be
validated item-by-item fall back to normal structural inference. Lua tables are
still mutable at runtime—the directive describes generated-source ownership,
not a runtime read-only wrapper.

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

## Share Types Without Runtime Imports

Use `export type` to make selected aliases public and `import type` to consume
them through a compile-time namespace:

This section focuses on the module boundary. For the complete type model,
including inference, table shapes, functions, unions, narrowing, and practical
annotation guidance, see [Typing In Tua](typing.md).

Put declaration-only APIs in `.d.tua` files. They are parsed, checked, indexed,
and watched like other Tua sources, but builds never create a Lua file or source
map for them.

```tua title="src/shared_types.d.tua"
export type Room = {
  name: string,
  spawnX: number,
  spawnY: number,
}

export type Player = {
  x: number,
  y: number,
  health: number,
}

type GeneratorScratch = { id: number }
```

```tua title="src/world.tua"
import type Shared from "./shared_types"

local function enter(room: Shared.Room, player: Shared.Player)
  player.x = room.spawnX
  player.y = room.spawnY
end
```

Import selected names when a namespace would be noisy, and re-export types to
build a public declaration facade:

```tua title="src/game_types.d.tua"
export type { Player, Room as LevelRoom } from "./shared_types"
```

```tua title="src/hud.tua"
import type { Player, LevelRoom as Room } from "./game_types"

local function drawHud(player: Player, room: Room)
end
```

Unlike `require(...)` or runtime `import(...)`, `import type` creates no local,
module load, or side effect. A regular `.tua` file erases type declarations from
its generated Lua; a `.d.tua` file has no generated output at all and rejects
runtime statements.
Only aliases declared with `export type` are visible. Private or missing aliases
produce an unknown-type diagnostic, missing modules use the normal project
module diagnostic, and aliases from imported namespaces work in locals,
parameters, returns, arrays, and nested table shapes.

Imported names cannot collide with runtime locals or local type aliases.
Export visibility is consistent for type-only imports and types reached through
runtime module aliases: cross-module Tua aliases must use `export type`.

Type-only dependencies participate in navigation and symbol-level cache
invalidation but do not create runtime require-cycle warnings. Editing one
exported alias rebuilds consumers of that alias rather than every consumer of
the declaration module. Recursive structural types may be split across modules;
pure re-export cycles receive a dedicated diagnostic. Namespace imports remain
the clearest default because ownership stays visible at every use.

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
| `---@field` | Adds a typed field, optional field, visibility metadata, documentation, or a dictionary-style index signature such as `[string]`. |
| `---@alias` | Declares a reusable type alias or literal union, on one line or as a bare `---@alias Name` followed by `---\| member` lines. |
| `---@type` | Types an attached local or value where the relationship is statically clear. |
| `---@param` | Types and documents a function parameter. |
| `---@return` | Types and documents one or more function returns. |
| `---@overload` | Adds another callable signature to a known function. |
| `---@vararg` | Types extra `...` arguments. |

These annotations can contribute to checking, module export inference, hover,
completion, signature help, semantic highlighting, and parameter
documentation. Index signatures keep a class open to keyed values without
appearing as literal field names in completion.

### External Declaration Libraries

Third-party packages can keep their runtime and declarations separate. Add a
[`type_libraries`](configuration.md#declaration-only-type-libraries) entry and
Tua connects each `---@meta` identity to the configured runtime `require`
prefix. The declarations participate in module exports, aliases, classes,
overloads, hover, signature help, definition lookup, and completion, but never
enter emitted output.

For example, Feel can remain installed under `lib/feel` while its published
`types` directory describes `feel`, `feel.love`, and related declaration
modules. Mapping `feel` to `lib.feel` lets this source use those types:

```tua
local feel = require("lib.feel")

feel.define("button.press", {
  { kind = "emit", event = "sound", payload = { cue = "click" } },
  { kind = "animate", duration = 0.06, to = { scale = 0.92 } },
})
```

Table-constructor completion is expected-type driven rather than tied to Feel.
LuaCATS unions with literal discriminator fields suggest the available literal
values, narrow later fields after a discriminator such as `kind = "animate"`,
and contextually type callback parameters. Recursive aliases are cycle-safe:
Tua keeps the surrounding table or union shape but treats the recursive
back-edge, such as a nested `parallel.steps` sequence, as `any`. The same
behavior applies to any declaration-typed Lua table DSL.

Completion remains bounded by the declarations. A field typed as plain
`string` has no literal values to suggest, runtime registrations such as
`feel.define("button.press", ...)` are not indexed as name catalogs, and a
generic `table<string, number>` cannot infer keys from a particular runtime
target.

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
`---@nodiscard`. `---@deprecated` is resolved across local declarations and
module exports: uses report `TL4056`, hover shows optional replacement guidance,
and completion plus semantic tokens identify deprecated APIs. Other metadata-only
tags are preserved with source spans and documentation but do not currently
change type checking.

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
