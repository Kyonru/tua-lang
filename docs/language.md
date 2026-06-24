# Language

Tua is a Lua superset. Type syntax is erased before output, and runtime code
stays ordinary Lua.

## Primitive Types

```tua
local score: number = 0
local title: string = "Arena"
local alive: boolean = true
local current: Player | nil = nil
local dynamic: any = require("legacy")
```

Supported primitive names:

- `number`
- `string`
- `boolean`
- `nil`
- `any`

`any` is the gradual escape hatch. Assignments to and from `any` are accepted.

## Type Aliases

```tua
type Vec2 = {
  x: number,
  y: number,
}
```

Aliases are erased from emitted Lua.

## Table Shapes

```tua
type Player = {
  pos: Vec2,
  hp: number,
  alive: boolean,
}
```

Annotated initializers are checked structurally:

```tua
local player: Player = {
  pos = { x = 10, y = 20 },
  hp = 100,
  alive = true,
}
```

## Optional Fields

```tua
type SaveSlot = {
  name?: string,
  level: number,
}
```

Optional target fields may be absent. Present optional fields must still match
their declared type.

## Union Types

```tua
type MaybeNumber = number | nil
```

A value can be assigned to a union when it is compatible with at least one union
member.

## LuaDoc Annotations

Tua can parse a LuaHelper/LuaLS-style annotation subset in both `.tua` and
`.lua` files:

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

Supported tags include `---@class`, `---@field`, `---@alias`, `---@type`,
`---@param`, `---@return`, `---@generic`, and `---@version`. Tua uses these
annotations for type environments, module export inference, hover, completion,
and signature help where the surrounding Lua is simple enough to analyze.

Malformed annotation diagnostics are controlled by `doc_annotations` in
`tua.toml`.

## Function Annotations

```tua
local function damage(player: Player, amount: number): Player
  player.hp -= amount
  return player
end
```

Emitted Lua:

```lua
local function damage(player, amount)
  player.hp = player.hp - amount
  return player
end
```

Tua checks known argument counts, known argument types, and annotated return
types.

## LOVE Asset Helpers And Methods

Tua includes built-in LOVE knowledge for common game code. Asset loader helpers
infer LOVE object types and emit ordinary LOVE API calls:

```tua
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
```

Emitted Lua:

```lua
local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")
```

Values inferred as LOVE objects expose method completions, hover, and signature
help:

```tua
player:setFilter("nearest")
local width = player:getWidth()
jump:play()
```

Prefer `:` for LOVE object methods. These methods usually need `self`, and Lua
passes `self` only for colon calls. When Tua knows the receiver is a LOVE object,
dot-style calls such as `player.setFilter("nearest")` produce warning `TL4018`
with a suggested colon call. Editor completions for LOVE methods also prefer
colon-call snippets; accepting a method completion after `player.setF` can
replace the dot with `:`.

## Compound Assignment

Supported operators:

- `+=`
- `-=`
- `*=`
- `/=`
- `%=`

Supported left-hand sides:

- simple names, such as `score`;
- plain field chains, such as `player.hp` or `world.player.hp`.

```tua
score += 5
player.hp -= amount
```

Emitted Lua:

```lua
score = score + 5
player.hp = player.hp - amount
```

Complex left-hand sides such as calls and computed indexes are rejected.
