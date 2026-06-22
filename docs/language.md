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
