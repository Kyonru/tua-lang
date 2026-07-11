# Language

Tua is a typed Lua superset for LuaJIT and LOVE projects. It keeps Lua as the
runtime language, adds erased type syntax and a few game-friendly conveniences,
then emits readable Lua 5.1-compatible source.

The language is intentionally gradual: code can be fully typed where static
feedback matters, lightly annotated around module boundaries, or left as normal
dynamic Lua. Unknown and highly dynamic values fall back to `any` instead of
forcing the whole project into a strict type system.

## Overview

Tua source is still Lua in the places where runtime behavior matters:

- functions, tables, loops, conditionals, closures, varargs, and `require`
  follow normal Lua rules;
- emitted files are plain `.lua` files with no Tua runtime dependency;
- existing `.lua` modules can live beside `.tua` modules and be copied into the
  build output unchanged;
- LuaDoc annotations in `.lua` files can provide type information to `.tua`
  callers when the module shape is simple enough to analyze.

Tua adds static-only language information on top:

- type aliases and table shapes describe the shape of Lua values;
- local and function annotations give the checker useful boundaries;
- unions and optional fields model common Lua `nil` flows;
- literal string unions and enum sugar model closed sets of string choices;
- compound assignments lower to ordinary Lua assignments;
- LOVE asset helpers infer common LOVE object types and lower to ordinary LOVE
  constructor calls.

The compiler checks what it can prove and stays permissive where Lua is
intentionally dynamic. That means a typed table field, known function argument,
or LOVE method call can produce a useful diagnostic, while metatables, `_G`,
runtime-built tables, and unknown `require` results can remain flexible.

## Source-To-Source Model

Tua does not introduce a new VM, bytecode format, package manager, or runtime
object model. A successful build turns:

```tua
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

local player = makePlayer("Ada")
player.hp -= 10
```

into readable Lua:

```lua
local function makePlayer(name)
  return {
    name = name,
    hp = 100,
  }
end

local player = makePlayer("Ada")
player.hp = player.hp - 10
```

Type aliases emit no runtime code. Function and local annotations disappear by
default; set `annotation_mode = "luacats"` in your project `tua.toml` to export
LuaCATS comments (`---@alias`, `---@type`, `---@param`, `---@return`, and stable
`---@field` exports) with emitted Lua. Sugar such as `-=` is lowered to Lua
5.1-compatible assignment.

## What Tua Tries To Know

Tua's semantic model is shared by the checker, hover, completion, definition,
signature help, inlay hints, references, rename, and other editor features. The
same inferred fact should show up consistently across those surfaces.

Useful static facts include:

- primitive literal types such as `number`, `string`, `boolean`, and `nil`;
- string literal types such as `"static"` and `"stream"`;
- local annotations and simple initializer inference;
- function parameter and return annotations;
- inferred returns for simple unannotated functions;
- multiple returns where known Lua or LOVE functions provide them;
- table-shape fields and optional fields;
- array-like table literals, string-key table indexing, and simple loop
  variables from numeric `for`, `ipairs`, and `pairs`;
- static prototype-style metatable inheritance and typed `__pairs`/`__ipairs`
  iterator facts where visible;
- module exports from simple `return { name = value }` tables;
- types exported through required `.tua` modules;
- selected LuaDoc/LuaLS-style annotations from interop files;
- Lua builtins and LOVE globals from built-in catalogs.

When a value cannot be proven, Tua prefers a conservative `any` or unknown
result. This keeps existing Lua code usable while still making typed code feel
useful.

## Literal Choices And Enums

Literal string unions are the core type feature for APIs with a small set of
valid string values:

```tua
type LoadMode = "static" | "stream"

local function load(path: string, mode: LoadMode)
  return sound(path, mode)
end
```

The checker accepts only the listed literal values where the expected type is
known, and the language server can complete those values inside string
arguments.

For longer LOVE-style mode lists, `enum` is available as erased sugar over the
same literal-union representation:

```tua
enum FilterMode
  "linear"
  "nearest"
end
```

This behaves like:

```tua
type FilterMode = "linear" | "nearest"
```

Enum declarations emit no runtime Lua. In LuaCATS emission mode they are
preserved as `---@alias FilterMode "linear" | "nearest"`.

## Modules And Interop

Tua preserves Lua's `require(...)` style for package-root modules, third-party
Lua libraries, and dynamic runtime loading.

```tua
local Player = require("player")
local player = Player.new("Ada")
```

For project-local modules, Tua also recognizes static import bindings:

```tua
local Player = import("./player")
local Utils = import("shared.utils")
```

`import` is not a keyword and it does not emit a runtime helper. In top-level
local bindings with one static string, `./` resolves child modules from the
current file while bare module names resolve through the configured module
roots. Parent traversal such as `../shared` is rejected; use a project module
name instead. Successful imports lower to ordinary Lua `require("...")`. If Tua
cannot map the import back to a runtime module name, it reports a diagnostic
instead of emitting a broken `import(...)` call.

The language server can resolve simple required and imported modules for hover,
completion, definition, references, document links, and diagnostics. The build
step copies plain `.lua` files and non-code assets into the output tree so a LOVE
project can produce a runnable distribution folder.

Lua interop is deliberately practical rather than magical. See
[Lua Interop](lua-interop.md) for LuaDoc/LuaLS-style annotations, required Lua
module inference, and mixed `.lua`/`.tua` project guidance.

## LOVE-Focused Layer

Tua is designed around LOVE projects, so the language server includes built-in
Lua and LOVE catalogs. Those catalogs power completions, hover, signature help,
simple call diagnostics, virtual definitions, and object method knowledge.

Tua also includes small source-level helpers for common asset loading:

```tua
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
local theme = music "assets/theme.ogg"
local ui = font("assets/ui.ttf", 18)
local wave = shader "assets/wave.glsl"
local pixels = imageData "assets/player.png"
local target = canvas(320, 180)
local frame = quad(0, 0, 16, 16, 64, 64)
local batch = spriteBatch(player, 128)
local sparks = particles(player, 64)
local position: Vec2 = vec2(100, 200)

player:setFilter("nearest")
jump:play()
```

Asset helpers lower to ordinary LOVE calls and infer LOVE object types for
editor tooling. `vec2(x, y)` lowers to a cached BrineVector constructor:

```lua
local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")
local theme = love.audio.newSource("assets/theme.ogg", "stream")
local ui = love.graphics.newFont("assets/ui.ttf", 18)
local wave = love.graphics.newShader("assets/wave.glsl")
local pixels = love.image.newImageData("assets/player.png")
local target = love.graphics.newCanvas(320, 180)
local frame = love.graphics.newQuad(0, 0, 16, 16, 64, 64)
local batch = love.graphics.newSpriteBatch(player, 128)
local sparks = love.graphics.newParticleSystem(player, 64)
local __tua_vec2 = require("brinevector")
local position = __tua_vec2(100, 200)

player:setFilter("nearest")
jump:play()
```

When generated Lua needs `brinevector`, `tua build` writes a bundled
`brinevector.lua` into the output directory unless the project already provides
one.

## Boundaries

Tua does not try to statically prove every Lua pattern. These remain gradual or
limited by design:

- dynamic metatable construction, computed `__index` chains, and runtime
  `getmetatable` mutation;
- arbitrary mutation of table shapes over time;
- values stored through `_G` or unknown globals;
- runtime-computed `require` paths;
- advanced LuaDoc generics and complex annotation forms;
- package management and dependency installation;
- runtime validation after code starts running.

Prefer annotations at module boundaries, constructors, returned module tables,
and LOVE-facing asset or object code. Leave highly dynamic glue as Lua. That is
the intended shape: typed where it pays rent, permissive where Lua is being Lua.

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

## LOVE Asset And Vector Helpers

Tua includes built-in LOVE knowledge for common game code. Asset loader helpers
infer LOVE object types and emit ordinary LOVE API calls:

```tua
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
local theme = music "assets/theme.ogg"
local ui = font("assets/ui.ttf", 18)
local wave = shader "assets/wave.glsl"
local pixels = imageData "assets/player.png"
local target = canvas(320, 180)
local frame = quad(0, 0, 16, 16, 64, 64)
local batch = spriteBatch(player, 128)
local sparks = particles(player, 64)
local position: Vec2 = vec2(100, 200)
```

Emitted Lua:

```lua
local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")
local theme = love.audio.newSource("assets/theme.ogg", "stream")
local ui = love.graphics.newFont("assets/ui.ttf", 18)
local wave = love.graphics.newShader("assets/wave.glsl")
local pixels = love.image.newImageData("assets/player.png")
local target = love.graphics.newCanvas(320, 180)
local frame = love.graphics.newQuad(0, 0, 16, 16, 64, 64)
local batch = love.graphics.newSpriteBatch(player, 128)
local sparks = love.graphics.newParticleSystem(player, 64)
local __tua_vec2 = require("brinevector")
local position = __tua_vec2(100, 200)
```

Values inferred as LOVE objects expose method completions, hover, and signature
help:

```tua
player:setFilter("nearest")
local width = player:getWidth()
jump:play()
```

Asset loader string paths also participate in editor tooling. In a configured
project, completion inside helpers with asset path arguments, such as
`image "..."`, `sound "..."`, `music "..."`, `font("...", 18)`, `shader "..."`,
and `imageData "..."`, suggests supported project asset paths filtered by helper
kind. Hovering an asset path shows the asset kind, relative path, inferred LOVE
type, validation status, extension, and cheap metadata when Tua can read it
without running game code. Today that optional binary metadata is
conservative; for example, valid PNG headers can provide dimensions, while audio
duration is left unknown.

`Vec2` is a built-in table-shaped type for common position and velocity code.
The constructor has signature `function vec2(x: number, y: number): Vec2`.
Fields `position.x` and `position.y` complete and hover as `number`. Tua also
models common BrineVector properties and operations such as `length`,
`normalized`, `dot`, `rotated`, and vector arithmetic. BrineVector uses
LuaJIT/FFI when appropriate and falls back to table vectors on platforms where
that is a better runtime choice.

Prefer `:` for LOVE object methods. These methods usually need `self`, and Lua
passes `self` only for colon calls. When Tua knows the receiver is a LOVE object,
dot-style calls such as `player.setFilter("nearest")` produce warning `TL4018`
with a suggested colon call. Editor completions for LOVE methods also prefer
colon-call snippets; accepting a method completion after `player.setF` can
replace the dot with `:`.

The editor can also help with common LOVE declarations and helpers. Completion
after `function love.` can insert callback snippets such as `love.update(dt)`,
`love.draw()`, and `love.conf(t)`. Inside `love.conf(t)`, nested config fields
such as `t.window.title`, `t.window.width`, and `t.modules.physics` complete.
Code actions can apply the TL4018 colon-call fix and can rewrite obvious
constructors such as `love.graphics.newImage("path")` to `image "path"`.

## Practical Metatable Typing

Tua recognizes a small, conservative subset of common Lua metatable patterns.
This is meant for editor feedback and obvious checks, not for proving arbitrary
runtime object systems.

Known function-valued table fields can be called with either explicit self or
colon syntax. Tua infers anonymous function-valued fields as function fields,
so hovering `lookupObject` shows `y` as a function rather than `any`:

```tua
local lookupObject = {
  count = 0,
  y = function(self, amount)
    self.count += amount
    return self.count
  end,
}

lookupObject.y(lookupObject, 1)
lookupObject:y(2)
```

Colon-defined methods with annotations get a typed receiver internally. For
example, `function Account:withdraw(amount: number): number` is checked as
though it had a leading `self: Account` parameter when a type alias named
`Account` exists. The emitted Lua still uses the original colon method syntax.

For static `setmetatable(object, metatable)` calls where both sides are known
tables, Tua propagates useful metatable facts into the object shape. The current
subset powers checker, hover, completion, go to definition, and inlay-style type
inference for:

- table/function `__index` facts when simple string keys are visible;
- self-index class tables such as `Account.__index = Account`, including
  methods declared with `function Account:withdraw(amount)`;
- prototype-style inheritance chains such as
  `setmetatable(Sub, { __index = Base })` when each link is statically visible;
- constructor methods such as `Account:new(...)` when their body visibly
  returns `setmetatable(instance, self)`;
- `__newindex` assignment contracts for simple visible forms such as
  `__newindex = function(t, key: "label", value: string) ... end` and
  `__newindex = { label = string }`, while unknown keys stay permissive;
- visible arithmetic metamethods `__add`, `__sub`, `__mul`, and `__div`;
- visible comparison metamethods `__eq`, `__lt`, and `__le`;
- visible `__len` for `#value`;
- visible `__call`, with the table receiver treated as the hidden first
  metamethod argument;
- visible `__pairs` and `__ipairs`, when the function return type is a
  tuple-like key/value pair such as `---@return [string, number]`;
- `tostring(value)` as `string`, including values with `__tostring`.

Dynamic metatable construction remains gradual. If the base table, metatable,
computed key, runtime iterator triple, or later mutation cannot be seen
statically, Tua falls back to permissive behavior instead of guessing.

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
