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
roadmap includes LuaHelper/LuaLS-style annotation stub ingestion so Tua can use
richer project-local Lua docs too.

## Practical Guidance

- Keep module returns explicit when you want editor tooling to understand them.
- Prefer returned table literals or locals containing table literals for module
  exports.
- Add Tua annotations at call boundaries where dynamic Lua and typed Tua meet.
- Use `any` intentionally for values that are truly dynamic.
