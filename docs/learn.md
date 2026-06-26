# Learn Tua

This page is a guided path through Tua. Use it when you want the language model
before reaching for the reference pages.

Tua is easiest to understand as Lua with typed editor feedback. You still write
Lua-shaped code, keep normal `require(...)` modules, run on LuaJIT/LOVE, and
ship plain `.lua` output. Tua adds erased type syntax, a few safe conveniences,
and LOVE-aware tooling on top.

## 1. Start With Normal Lua

Most Lua code is already valid Tua:

```tua
local function makePlayer(name)
  return {
    name = name,
    hp = 100,
  }
end

local player = makePlayer("Ada")
print(player.name)
```

This is still dynamic Lua. If Tua cannot prove a value's shape, it stays
permissive instead of guessing.

## 2. Add Types At Boundaries

The first useful move is typing the data that crosses module or function
boundaries:

```tua
type Player = {
  name: string,
  hp: number,
  alive: boolean,
}

local function makePlayer(name: string): Player
  return {
    name = name,
    hp = 100,
    alive = true,
  }
end
```

The emitted Lua removes the type syntax:

```lua
local function makePlayer(name)
  return {
    name = name,
    hp = 100,
    alive = true,
  }
end
```

Use annotations where they buy clarity: constructors, public module functions,
state tables, configuration tables, and gameplay entities.

## 3. Model Lua `nil` Honestly

Lua code often uses `nil` for missing state. Tua models that with unions and
optional fields:

```tua
type SaveSlot = {
  name?: string,
  player: Player | nil,
}

local slot: SaveSlot = {
  player = nil,
}
```

Optional fields may be absent. Union values are accepted when they match one of
the members.

## 4. Model Closed String Choices

Many Lua and LOVE APIs use strings as modes. Tua can make those choices
explicit:

```tua
type LoadMode = "static" | "stream"

enum FilterMode
  "linear"
  "nearest"
end

local function configure(mode: LoadMode, filter: FilterMode)
end

configure("static", "nearest")
```

Literal unions are the underlying type feature. `enum` is just erased sugar for
longer lists, so it emits no runtime table or constants.

## 5. Keep Modules Lua-Shaped

Tua preserves Lua's module style:

```tua
-- src/player.tua
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

```tua
-- src/main.tua
local Player = require("player")

local player = Player.makePlayer("Ada")
player.hp -= 10
```

Simple returned module tables give the language server enough information for
hover, completion, go to definition, references, and rename.

## 6. Use LuaDoc For Lua Interop

Plain `.lua` files can stay dynamic, or they can expose useful type information
with LuaDoc/LuaLS-style annotations:

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

Then a `.tua` caller can get typed hover, completion, and diagnostics when the
module is resolved:

```tua
local Enemies = require("enemies")

local enemy = Enemies.makeEnemy()
enemy.hp -= 5
```

See [Lua Interop](lua-interop.md) for the supported annotation subset and
interop rules.

## 7. Lean Into LOVE Helpers

Tua includes a built-in LOVE API catalog and small asset loader helpers:

```tua
local playerImage = image "assets/player.png"
local jumpSound = sound "assets/jump.wav"
local position: Vec2 = vec2(100, 200)

playerImage:setFilter("nearest")
jumpSound:play()
```

These lower to ordinary LOVE calls:

```lua
local playerImage = love.graphics.newImage("assets/player.png")
local jumpSound = love.audio.newSource("assets/jump.wav", "static")
local __tua_vec2 = require("brinevector")
local position = __tua_vec2(100, 200)

playerImage:setFilter("nearest")
jumpSound:play()
```

Because Tua knows `playerImage` is an `Image` and `jumpSound` is a `Source`, the
editor can show LOVE method completions, hovers, and signature help. Tua can
also warn about obvious missing or unsupported asset paths.

`Vec2` is built in for common LOVE position and velocity code. The editor can
complete and hover `position.x` and `position.y` as `number`, and emitted Lua is
backed by bundled BrineVector. BrineVector uses LuaJIT/FFI for fast desktop
LOVE builds and falls back to table vectors where JIT/FFI is not the right fit.

## 8. Prefer `:` For LOVE Methods

LOVE object methods usually need Lua's colon-call syntax so `self` is passed:

```tua
playerImage:setFilter("nearest")
```

Dot-style calls can be wrong:

```tua
playerImage.setFilter("nearest")
```

When Tua knows the receiver is a LOVE object, it warns about the dot call and
editor completions prefer colon-call snippets.

## 9. Let Dynamic Lua Stay Dynamic

Tua is not trying to make Lua stop being Lua. These are fine places to stay
permissive:

```tua
local plugin: any = require("runtime_plugin")
local value = plugin.makeSomething(dynamicInput)
```

Use `any` deliberately when a value is runtime-shaped, third-party, or
metatable-heavy. Add annotations later if that code becomes important to the
editor or checker.

## 9. Build And Inspect The Output

The normal loop is:

```bash
cargo run -p tua-cli -- check
cargo run -p tua-cli -- build
```

`tua build` mirrors the configured source tree into the output directory:

- `.tua` files become `.lua`;
- `.lua` files are copied unchanged;
- assets are copied unchanged;
- `require(...)` calls stay as Lua requires.

That makes the output folder a runnable LOVE distribution folder.

## 10. Try The Examples

The feature tour is the best place to see language and editor behavior in one
project:

```bash
make build-features
```

For LOVE projects:

```bash
make build-love-minimal
make build-love-full
```

Open `examples/features` in VS Code when working on editor features. Open the
LOVE examples when checking emitted game output.

## Learning Order

1. [Quickstart](quickstart.md): create, check, and build a tiny project.
2. [Language](language.md): read the syntax and emission reference.
3. [Lua Interop](lua-interop.md): learn how `.lua`, `.tua`, and LuaDoc work
   together.
4. [Language Server](lsp.md): see what the editor should provide.
5. [Examples](examples.md): run the bundled projects.
6. [Architecture](architecture.md): read this when changing the implementation.
