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
`love2d-community/love-api` for completions, hover, and signature help. The
parser also understands a LuaHelper/LuaLS-style annotation subset, so plain Lua
files can provide typed functions, aliases, fields, documentation, and simple
module exports to `.tua` callers.

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

Tua also includes small typed LOVE asset loader helpers:

```tua
local player: Image = image "assets/player.png"
local jump: Source = sound "assets/jump.wav"
```

They emit plain LOVE calls in generated Lua:

```lua
local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")
```

Values inferred as `Image` or `Source` expose common LOVE object methods in
completion, such as `player:getWidth()` or `jump:play()`.

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
