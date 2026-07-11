# Lua Interop

Tua is designed for existing Lua projects. It preserves normal Lua `require`
semantics and emits plain Lua files.

## Requiring Lua From Tua

```tua
local Legacy = require("legacy")

local clamped = Legacy.clamp(score, 0, 100)
```

Tua can resolve required `.lua` files for navigation and module export inference
when the module shape is simple enough to analyze.

Dynamic Lua remains permissive. Unknown values usually become `any` or stay
quiet instead of producing speculative diagnostics.

## Static Imports Between Tua Modules

Tua files can bind project modules with a static `import(...)` helper:

```tua
local Player = import("./player")
local Shared = import("shared")
```

This is compile-time sugar only. Child paths such as `./player` resolve from the
current source file; cross-folder modules use project module names such as
`shared` or `engine.foo`. Parent traversal such as `../shared` is rejected. Static
imports participate in the same module graph as `require(...)` and emit normal
Lua `require("...")` calls. Use `require(...)` directly for external Lua
packages and runtime-computed module names.

## Requiring Tua From Lua

Tua builds `.tua` files into `.lua` files under the configured output directory.
Lua code should require the emitted module path at runtime.

Source:

```tua
-- src/player.tua
local function makePlayer(name: string)
  return { name = name, hp = 100 }
end

return {
  makePlayer = makePlayer,
}
```

Output:

```lua
-- build/player.lua
local function makePlayer(name)
  return { name = name, hp = 100 }
end

return {
  makePlayer = makePlayer,
}
```

Lua:

```lua
local Player = require("player")
local player = Player.makePlayer("Ada")
```

## Builtins And Stubs

Tua includes a curated Lua catalog and a built-in LOVE catalog generated from
`love2d-community/love-api` for completions, hover, signature help, enum-like
string values, and LOVE object methods. The parser also understands a
LuaHelper/LuaLS-style annotation subset, so plain Lua files can provide typed
functions, aliases, fields, documentation, and simple module exports to `.tua`
callers.

## LuaDoc Annotations

Tua can parse a LuaHelper/LuaLS-style annotation subset in both `.lua` and
`.tua` files. In practice, annotations are most useful at interop boundaries:
plain Lua modules, third-party libraries, returned module tables, and functions
called from typed Tua code.

Supported tags include:

- `---@class`
- `---@field`
- `---@alias`
- `---@type`
- `---@param`
- `---@return`
- `---@overload`
- `---@vararg`
- `---@generic`
- `---@version`
- `---@enum`
- `---@meta`
- `---@module`
- `---@diagnostic`
- `---@cast`
- `---@as`
- `---@deprecated`
- `---@nodiscard`

Tua uses supported annotations for type environments, module export inference,
hover, completion, signature help, and parameter documentation where the
surrounding Lua is simple enough to analyze.

Supported type forms include primitive names, aliases, unions, string literal
types such as `"static" | "stream"`, optional shorthand such as `number?`,
arrays such as `Image[]`, tuples such as `[number, number]`, dictionaries such
as `{ [string]: Source }`, key-value tables such as `table<string, Image>`, and
function types such as `fun(path: string): Image`. The LuaHelper `identifier`
type keyword is treated as `string`; Tua does not currently model it as a
separate name-like type. Inside `---@class` fields and annotations attached to
named colon methods, `Self` resolves to the class or receiver type being
described.

`---@overload` entries add extra call signatures for a known function. Signature
help can show and select those overloads, and call diagnostics use the matching
overload when Tua can prove one applies. `---@vararg T` types extra `...`
arguments for annotated vararg functions.

Some LuaLS tags are currently preserved as metadata with spans and docs but do
not change type checking yet. This includes `---@meta`, `---@module`,
`---@diagnostic`, `---@cast`, `---@as`, `---@deprecated`, and `---@nodiscard`.
They are parsed so editor features can grow around them without making current
Lua code fail.

```lua
---@alias Health number

---@class Player
---@field name string
---@field hp Health

---@param name string player name
---@return Player created player
local function makePlayer(name)
  return {
    name = name,
    hp = 100,
  }
end
```

Literal unions are useful for APIs with a small set of valid string values:

```lua
---@alias LoadMode "static" | "stream"

---@param path string
---@param mode LoadMode
---@return Image
local function loadImage(path, mode)
  return love.graphics.newImage(path)
end
```

When called from Tua, `"static"` and `"stream"` are accepted, other literal
strings are rejected, and the editor can complete valid values inside string
arguments.

Overloads are useful for Lua libraries that accept a few distinct call shapes:

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

Typed varargs keep flexible Lua functions checked without making them rigid:

```lua
---@param message string
---@vararg identifier
local function log(message, ...)
  print(message, ...)
end
```

Malformed annotation diagnostics are controlled by `doc_annotations` in
`tua.toml`.

### Annotated Lua Module Example

```lua
---@param value number
---@param min number
---@param max number
---@return number clamped
local function clamp(value, min, max)
  if value < min then
    return min
  end

  if value > max then
    return max
  end

  return value
end

return {
  clamp = clamp,
}
```

When this file is required from Tua, the language server can use the annotated
signature for hover, completion, signature help, and argument diagnostics.

```tua
local Legacy = require("legacy")

local score = Legacy.clamp(125, 0, 100)
```

### Annotated Types From Lua

Lua modules can also provide named table shapes to Tua callers:

```lua
---@class Enemy
---@field name string
---@field hp number

---@return Enemy
local function makeEnemy()
  return {
    name = "slime",
    hp = 20,
  }
end

return {
  makeEnemy = makeEnemy,
}
```

```tua
local Enemies = require("enemies")

local enemy = Enemies.makeEnemy()
enemy.hp -= 5
```

The analyzer does not run Lua code to discover symbols. Keep module returns
explicit when you want editor tooling to understand exports.

## LOVE Interop Helpers

Tua also includes small typed LOVE asset loader helpers:

```tua
local player: Image = image "assets/player.png"
local jump: Source = sound "assets/jump.wav"
local theme: Source = music "assets/theme.ogg"
local ui: Font = font("assets/ui.ttf", 18)
local wave: Shader = shader "assets/wave.glsl"
local position: Vec2 = vec2(100, 200)
```

They emit plain LOVE calls in generated Lua:

```lua
local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")
local theme = love.audio.newSource("assets/theme.ogg", "stream")
local ui = love.graphics.newFont("assets/ui.ttf", 18)
local wave = love.graphics.newShader("assets/wave.glsl")
local __tua_vec2 = require("brinevector")
local position = __tua_vec2(100, 200)
```

Values inferred as LOVE API objects expose generated object methods in
completion, such as `player:getWidth()`, `jump:play()`, or `file:open("r")`.

`Vec2` is a built-in table-shaped type for position and velocity values. The
`vec2(x, y)` constructor type-checks number arguments, returns `Vec2`, and emits
BrineVector-backed Lua. `tua build` bundles `brinevector.lua` into the output
folder when needed, unless the project already provides that module.

Use colon-call syntax for LOVE object methods:

```tua
player:setFilter("nearest")
jump:play()
```

When Tua can prove the receiver is a LOVE object, dot-style calls such as
`player.setFilter("nearest")` produce warning `TL4018`. LOVE method
completions prefer colon-call snippets and can rewrite the separator when a
completion is accepted after `.`.

## Practical Guidance

- Keep module returns explicit when you want editor tooling to understand them.
- Prefer returned table literals or locals containing table literals for module
  exports.
- Add Tua annotations at call boundaries where dynamic Lua and typed Tua meet.
- Use `any` intentionally for values that are truly dynamic.
