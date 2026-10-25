---
icon: lucide/braces
---

# Language

Tua is a typed Lua superset for LuaJIT and LOVE projects. It keeps Lua as the
runtime language, adds erased type syntax and a few game-friendly conveniences,
then emits readable Lua 5.1-compatible source.

The language is intentionally gradual: code can be fully typed where static
feedback matters, lightly annotated around module boundaries, or left as normal
dynamic Lua. Unknown and highly dynamic values fall back to `any` instead of
forcing the whole project into a strict type system.

The parser library's Luau-capable grammar is an internal implementation detail.
After parsing, Tua rejects non-Tua constructs with `TL0004` before checking or
emission. In particular, use Tua annotations instead of `value :: Type`, and
rewrite `continue`, Luau conditional expressions, and interpolated strings with
documented Tua or Lua 5.1-compatible forms. Every completed output file is also
parsed as Lua 5.1; `TL2010` indicates that compiler lowering or emission broke
that final invariant.

!!! tip "New to Tua?"
    This page is a feature reference. If you are writing your first Tua
    project, start with [Learn Tua](learn.md) and return here when you need exact
    syntax or behavior. For a focused explanation of inference, narrowing,
    shared aliases, and declaration modules, see [Typing In Tua](typing.md).

## Names And Keywords

Reserved keywords such as `and`, `or`, and `end` cannot be used as variable,
function, or parameter names. In these name positions, `TL0001` identifies the
keyword, for example: ``reserved keyword `and` cannot be used as a variable name``.
Use a different identifier, such as `condition`, `either`, or `finish`.

Built-in names such as `type` and `print` are ordinary identifiers and can be
shadowed by locals or parameters. Keyword text is also valid as a quoted table
key: `record["end"]`.

## Find A Feature

### Types And Data

- [Primitive types and `any`](#primitive-types): `number`, `string`, `boolean`,
  `nil`, and the dynamic escape hatch.
- [Local inference and reassignment](#inferred-local-reassignment): inferred
  binding contracts, widening, and reassignment checks.
- [Type aliases and `Self`](#type-aliases): reusable erased type names and
  recursive shapes.
- [Table shapes](#table-shapes) and [optional fields](#optional-fields): typed
  Lua records and `field?: Type`.
- [Arrays, dictionaries, and LuaDoc tuples](#arrays-dictionaries-and-luadoc-tuples): `T[]`,
  `{ [Key]: Value }`, and `[T, U]`.
- [Union types](#union-types): `T | U` values and nilable state.
- [Literal choices and enums](#literal-choices-and-enums): exact string choices
  and erased enum declarations.
- [Runtime `is` checks](#runtime-is-checks): runtime-compatible checks with
  branch narrowing.

### Functions And Objects

- [Functions, parameters, and returns](#functions-parameters-and-returns):
  annotations, defaults, optional parameters, and typed varargs.
- [Function types](#function-types): `(value: T) -> U` callbacks and named
  signatures, including multiple Lua returns.
- [Classes](#classes): fields, constructors, `self`, inheritance, `super`,
  `override`, and final dispatch.
- [Practical metatable typing](#practical-metatable-typing): conservative
  support for common Lua object and metamethod patterns.
- [Compound assignment](#compound-assignment): `+=`, `-=`, `*=`, `/=`, and
  `%=` lowering.

### Modules And Lua

- [Modules and interop](#modules-and-interop): `require(...)`, static
  `import(...)`, exported module tables, and mixed `.lua`/`.tua` projects.
- [Source-to-source emission](#source-to-source-model): erased types, readable
  Lua output, LuaCATS emission, and runtime compatibility.
- [Gradual-analysis boundaries](#boundaries): patterns that intentionally stay
  dynamic or conservative.

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

## Semantic Model

Tua's semantic model is shared by the checker, hover, completion, definition,
signature help, inlay hints, references, rename, and other editor features. The
same inferred fact should show up consistently across those surfaces.

Useful static facts include:

- primitive literal types such as `number`, `string`, `boolean`, and `nil`;
- string literal types such as `"static"` and `"stream"`;
- local annotations and simple initializer inference;
- function parameter and return annotations;
- native function types for aliases, fields, callbacks, and typed varargs;
- scoped `Self` in type aliases and named colon-method annotations;
- trailing default and optional parameters such as `duration: number = 0.3`
  and `easing?: string`;
- inferred returns for simple unannotated functions;
- multiple returns where known Lua or LOVE functions provide them;
- table-shape fields and optional fields;
- `T[]` arrays, including class fields initialized with `{}`, array-like table
  literals, string-key table indexing, and simple loop variables from numeric
  `for`, `ipairs`, and `pairs`;
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

## LOVE APIs And Game Runtime Helpers {#love-focused-layer}

### LOVE API Catalog

Tua is designed around LOVE projects, so the language server includes built-in
Lua and LOVE catalogs. Those catalogs power completions, hover, signature help,
simple call diagnostics, virtual definitions, and object method knowledge.

Both `math` and `love.math` also expose a small opt-in game-math set:

```tua
local angle: number = math.tau
local health: number = math.clamp(rawHealth, 0, 100)
local x: number = love.math.lerp(startX, targetX, amount)
local orange = love.math.rgb(255, 128, 0)
local faded = love.math.rgba(40, 80, 255, 128)
```

The complete set is `tau`, `clamp`, `lerp`, `inverseLerp`, `remap`, `sign`,
`round`, `smoothstep`, `rgb`, and `rgba`. Each helper adds a small guarded Lua
definition only to a generated file that uses it. See
[Game Math Helpers](vectors.md#game-math-helpers) for every signature and
usage examples.

### Feature Guides

Tua's game-focused helpers are documented separately so this page can remain a
language reference:

- [Sprites And Animation](sprites.md)
- [Vectors And Game Math](vectors.md)
- [Signals](signals.md)
- [State Stores](stores.md)
- [State Machines](state-machines.md)
- [Input](input.md)
- [Tweening](tweening.md)
- [Camera](camera.md)
- [Entity Component Systems](ecs.md)
- [Object Pooling](object-pooling.md)
- [Audio Systems](audio.md)

Helpers lower to ordinary LOVE calls or bundled Lua modules. The build includes
a helper runtime only when the source uses that feature, and a compatible
project-provided module can replace the bundled implementation.

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

## Inferred Local Reassignment

In `.tua` files, an unannotated local with a statically known initializer gets
one stable, widened type for that lexical binding:

```tua
local score = 10       -- number
score = 12             -- accepted
score = "high"         -- TL1003: expected number, found string

local state = "idle"   -- string, not the literal type "idle"
state = "running"      -- accepted
```

Assignments inside `if`, loop, and nested blocks are checked against the same
binding contract. A branch may narrow a value temporarily, but it does not
widen the local after control flow joins. A shadowing `local` starts a separate
contract.

Lua initializes declarations without a value to `nil`, so nil-only
initialization remains explicit:

```tua
local pending = nil
pending = 10 -- TL1003: expected nil, found number

local optional: number | nil = nil
optional = 10 -- accepted

local dynamic: any = nil
dynamic = "loaded" -- accepted
```

Unknown expressions and values typed as `any` remain gradual. Ordinary `.lua`
files also keep normal permissive Lua reassignment behavior unless LuaDoc
annotations provide a contract. This changes checking only; emitted Lua still
performs ordinary runtime assignment.

## Type Aliases

```tua
type Vec2 = {
  x: number,
  y: number,
}
```

Aliases are erased from emitted Lua. Inside an alias body, `Self` resolves to
the alias currently being defined:

```tua
type Node = {
  parent: Self | nil,
}
```

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

## Arrays, Dictionaries, And LuaDoc Tuples

Use `T[]` for arrays. Tua does not require or provide a `Table<T>` generic:

```tua
local steps: number[] = {}
local names: string[] = { "major", "minor" }
local byName: { [string]: number } = {}
local scores: { [string]: number } = { ada = 10, mina = 12 }
type Position = { x: number, y: number }
```

Native Tua annotations do not currently provide fixed-length positional table
types. Lua interop still accepts LuaCATS tuple annotations such as
`---@alias Position [number, number]`. Use a record when positions have names,
or an array when its length is dynamic.

An empty table literal is accepted as the initial value of an annotated array
or dictionary. A record literal with compatible named string fields is also
accepted for a string-keyed dictionary. Once annotated, writes, indexed reads,
`table.insert`, `ipairs`, and supported editor features use the declared
element type. Parentheses are required when the array element is a union or
function type:

```tua
local values: (number | string)[] = {}
local callbacks: ((dt: number) -> nil)[] = {}
```

An ordinary indexed read assumes the dense-array contract and has type `T`.
Dictionary reads instead return `Value | nil`, including dot access, since
dictionary keys are not guaranteed to exist.
Use `rawget(values, index)` when an index may be absent; its result is `T |
nil`. Tua also permits direct checks such as `values[index] ~= nil`, since Lua
can return `nil` for an out-of-range index even when the declared element type
is non-nilable.

## Union Types

```tua
type MaybeNumber = number | nil
```

A value can be assigned to a union when it is compatible with at least one union
member.

Nilable locals, including mutable state captured from an outer scope, narrow
inside truthy branches and after terminating guard clauses:

```tua
local active: Piece | nil = nil

local function update()
  if not active then
    return
  end

  active.x += 1 -- active is Piece here
end
```

`if active == nil then return end` has the same effect. The postcondition is
cleared by a later direct assignment to `active`; a nonterminating branch does
not establish it. When mutation is intentionally indirect or difficult to
follow, copy the guarded value into a local before calling other code.

## Runtime `is` Checks

Use `value is Type` in `if`, `elseif`, and `while` conditions when a cheap
runtime check should also narrow the true branch:

```tua
local function label(value: string | number): string
  if value is string then
    return value
  end

  return "" .. value
end
```

Primitive targets compile to `type(value) == "..."`. The supported runtime
names are `string`, `number`, `boolean`, `nil`, and `table`.

For visible metatable class tables, Tua emits a direct identity check:

```tua
local Player = { health = 0 }
Player.__index = Player

if value is Player then
  value.health -= 1
end
```

Emitted Lua:

```lua
if getmetatable(value) == Player then
  value.health = value.health - 1
end
```

Aliases are erased at runtime, so `is` only accepts an alias when it resolves to
a supported primitive. Class targets use generated `__type_id` / `__is`
metadata, so inherited class checks are supported for class syntax. Erased
table-shape aliases, structural checks, manual-metatable inheritance checks, and
container targets such as `string[]` are not runtime-checkable in v1 and are
rejected before Lua is emitted.

## Classes

Classes are optimized syntax for common Lua metatable patterns:

```tua
open class Entity {
  x: number
  y: number

  init(x: number, y: number) {
    self.x = x
    self.y = y
  }

  update(dt: number) {
  }
}

final class Particle extends Entity {
  speed: number = 120
  trail: number[]

  init(x: number, y: number) {
    super(x, y)
    self.trail = {}
  }

  override final update(dt: number) {
    self.x += self.speed * dt
    table.insert(self.trail, self.x)
  }
}

local particle = Particle.new(10, 20)
particle:update(0.016)
```

`class` is final by default; `open class` is required for inheritance.
`final class` may be written explicitly. Instance fields, `init`, methods,
`override`, `final` methods, and `super(...)` are supported. Static members,
private members, getters/setters, generic classes, mixins, and multiple
inheritance are not part of v1.

Class fields use the same native types as locals and aliases, including `T[]`,
dictionaries, unions, function types, and scoped `Self`. Assigning `{}`
to an annotated array or dictionary field is valid. Fields not declared by the
class or an ancestor are rejected; use `any` only when a deliberately dynamic
object is required.

Method parameters and returns accept the same native type surface, including
table-shaped return types. Unrecognized text in a class body reports `TL1008`
instead of being discarded during class lowering.

Constructors return the declared class type, and methods keep their exact
parameter and return types across hover, completion, signature help, checking,
definition, outlines, workspace symbols, and static module exports. Static
`require` and `import` boundaries also preserve nominal class identity and
inheritance, so two structurally identical exported classes are not
interchangeable. Duplicate `init` declarations, inheritance cycles, extending
non-open classes, missing or spurious `override`, and override signature drift
are diagnosed.

Class names, fields, and methods participate in source definition, references,
and rename. This includes accesses through locally inferred instances and
instances exported from static modules; edits target the original class syntax,
not generated Lua. Dynamic values typed as `any` remain outside this guarantee.

Inherited fields may be redeclared only when their resolved type is unchanged.
Every field whose type excludes `nil` must also be initialized on every path
that returns from the constructor. A field default, a definite `self.field =`
assignment, or a guaranteed `super(...)` call can satisfy the contract;
assigning in only one conditional branch cannot. Fields whose type accepts
`nil` may remain absent, matching Lua's field semantics.

Those correctness checks are always-on `TL1008` errors. Standard class lints
separately report constructors that can return without `super(...)` (`TL4057`),
direct calls to overridable methods during an open class's initialization
(`TL4058`), and type-only inherited field redeclarations with no child default
(`TL4059`). They obey `diagnostics.rules` and exact-code suppressions. Empty
overrides, overrides that replace rather than call parent behavior, exported
methods with no local use, and open classes with no known local subclass remain
valid and do not warn.

Inside `init` and instance methods, `self` resolves to the enclosing class.
Completion after `self.` and member hover include declared and inherited fields
and methods with their exact types.

`init` supports typed varargs. Generated constructors forward every declared
parameter, including `...`, and standalone `super(...)` or `super.method(...)`
calls preserve their argument expressions. Member calls such as
`helper.super()` remain ordinary Lua calls. A subclass's constructor signature
is independent of its parent's signature; the arguments passed to
`super(...)` are checked separately against the parent initializer.

A child class may appear before its parent in the same file. Tua gives the
later parent a stable forward table, so inherited methods and `super` calls work
once the parent declaration has executed; source order does not cause the
parent declaration to run early.

Generated Lua uses ordinary tables, `__index`, local method functions, and
per-class `__type_id` / `__is` metadata. `Class.new(...)` constructs instances.
Within each class, the `init` body runs before that class's field defaults; this
ordering is preserved across inheritance and constructor optimization.
When a constructor is proven to contain only field defaults, `self.field = ...`,
and a flattenable `super(...)`, Tua emits a direct table constructor. Otherwise
it preserves explicit init calls. Calls to known final methods on known class
instances can lower to direct local function calls; non-final calls keep normal
virtual dispatch. Locally inferred native class instances accept both
`instance.method(...)` and `instance:method(...)`; use Lua's colon form for
instances obtained through module or dynamic boundaries so the receiver is
explicit in the emitted Lua. A dot call there would run without `self`, and
`TL1004` reports it with the colon call to use instead. Writing the colon form
everywhere keeps code correct when a class moves to another module.

## Functions, Parameters, And Returns

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
types. Trailing parameters can be optional or provide a Lua default value:

```tua
local function heal(player: Player, amount: number = 5): Player
  player.hp += amount
  return player
end

local function describe(player: Player, mode?: string): string
  if mode ~= nil then
    return mode
  end

  return player.name
end
```

`amount: number = 5` inserts the default expression when the argument is `nil`.
`mode?: string` accepts an omitted argument and has type `string | nil` inside
the function. Default and optional parameters must be trailing.

Use `...: Type` for typed varargs:

```tua
local function logAll(prefix: string, ...: string)
  print(prefix, ...)
end
```

## Function Types

Native callback types use `(parameters) -> returns`:

```tua
type Formatter = (value: number, ...string) -> string

type Hooks = {
  format: Formatter,
  decode: (text: string) -> (boolean, string),
}

local formatter: Formatter = function(value: number, ...: string): string
  return tostring(value)
end

local hooks: Hooks = {
  format = formatter,
  decode = function(text: string): (boolean, string)
    return true, text
  end,
}
```

The same representation is used by aliases, table fields, callback
annotations, module exports, checking, hover, completion, inlay hints, and
signature help. Parameter names are preserved for editor presentation but do
not change type compatibility. Use `...Type` in a function type and
`...: Type` in a function declaration. A parenthesized return list represents
multiple Lua returns.

LuaDoc/LuaCATS function types such as
`fun(value: number, ...: string): string` lower to this same semantic type.
When `annotation_mode = "luacats"` is enabled, native callback aliases emit
compatible `---@alias ... fun(...)` annotations.

Generics are not part of Tua. Arrays, dictionaries, concrete aliases, and
gradual `any` cover the supported container and dynamic boundaries.

## LOVE Assets And Callbacks

Tua includes built-in LOVE knowledge for common game code. Asset loader helpers
infer LOVE object types and emit ordinary LOVE API calls:

### Asset Loaders

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
```

Values inferred as LOVE objects expose method completions, hover, and signature
help:

```tua
player:setFilter("nearest")
local width = player:getWidth()
jump:play()
```

Asset loader string paths also participate in editor tooling. In a configured
project, completion inside helpers and native LOVE constructors with asset path
arguments, such as `image "..."`, `sound "..."`, `font("...", 18)`,
`love.graphics.newImage("...")`, and `love.audio.newSource("...", "stream")`,
suggests supported project asset paths filtered by loader kind. Hovering an
asset path shows the asset kind, relative path, inferred LOVE type, validation
status, extension, and cheap metadata when Tua can read it without running game
code. Tua also reports obvious missing files, unsupported formats, and source
type mismatches such as using an audio file where an image is expected. Today
that optional binary metadata is conservative; for example, valid PNG headers
can provide dimensions, while audio duration is left unknown.

A local that holds a shader asset also checks the uniform names it is sent.
LOVE raises an error at runtime when `Shader:send` or `Shader:sendColor` names
a uniform the shader does not declare, so Tua reports `TL2013` for a literal
name that no `uniform` or `extern` declaration in the shader file matches:

```tua
local wave = shader "assets/wave.glsl" -- declares `extern number time;`
wave:send("time", love.timer.getTime())
wave:send("tim", 0) -- TL2013: did you mean `time`?
wave:send("light" .. tostring(index), color) -- needs a uniform such as `light2`
```

A name built around one value, as in `"light" .. index .. "Color"` or
`string.format("light%dColor", index)`, is expanded when the value comes from a
numeric `for` loop with literal bounds. Each generated name is checked, so a
loop that runs one step past the declared uniforms is reported:

```tua
for i = 1, 4 do -- the shader declares light1Color through light3Color
  wave:send("light" .. i .. "Color", { 1, 1, 1 }) -- TL2013: no `light4Color`
end
```

Other runtime values only require some declared uniform that fits the pattern.

LOVE also rejects a declared uniform that the shader never reads, because the
GLSL compiler removes it. Tua reports warning `TL2014` when such a uniform is
sent; read it in the shader, or guard optional uniforms with
`Shader:hasUniform`.

Tua reports error `TL2015` when a sent value is provably incompatible with the
uniform's GLSL type: scalars need numbers or booleans, vectors need component
tables, matrices accept flat tables or rows (`mat4` also accepts a LOVE
`Transform`), and texture uniforms need compatible textures or canvases.
Literal vector and matrix tables are checked for missing components;
`sendColor` requires `vec3` or `vec4` and accepts three or four components.

```tua
wave:send("time", {}) -- TL2015: time expects a number
wave:send("time", 0)  -- accepted
```

These checks preserve `any` and uncertain unions. Unknown table lengths are
not checked for component count. Numeric strings, raw `Data` sends, matrix
layout overloads, and extra values ignored by LOVE remain accepted. Texture
dimensions and raw-data sizes are not statically checked.

The editor also completes uniform names, shows their declarations and expected
Lua values on hover, and navigates to GLSL declarations (including both stage
sections). See [shader uniform editing](lsp.md#shader-uniform-editing).

Names held in variables are not checked, and neither are shaders whose
uniforms come from uniform blocks or preprocessor macros.

### LOVE Methods And Callbacks

Prefer `:` for LOVE object methods. These methods usually need `self`, and Lua
passes `self` only for colon calls. When Tua knows the receiver is a LOVE object,
dot-style calls such as `player.setFilter("nearest")` produce warning `TL4018`
with a suggested colon call. Editor completions for LOVE methods also prefer
colon-call snippets; accepting a method completion after `player.setF` can
replace the dot with `:`.

The editor can also help with common LOVE declarations and helpers. Completion
after `function love.` can insert callback snippets such as `love.update(dt)`,
`love.draw()`, and `love.conf(t)`, with generated callback and parameter
documentation. Callback declarations may omit unused Lua parameters, but close
name typos, required extra parameters, and explicitly incompatible parameter
types report `TL5006`. Code actions can rename close callback typos, apply the
TL4018 colon-call fix, and rewrite obvious constructors such as
`love.graphics.newImage("path")` to `image "path"`. Source actions can create
missing core `love.load`, `love.update`, and `love.draw` declarations, organize
or sort static imports, remove unused static import bindings, and fix all safe
diagnostics in the current file.

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
`Account` exists. `Self` in that method's parameter or return annotations
resolves to the same receiver type, so `function Account:chain(): Self` returns
`Account`. The emitted Lua still uses the original colon method syntax.

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

The right-hand side keeps its grouping, so `x *= a + b` emits
`x = x * (a + b)`.

When the target's type defines the operator through a metatable, the
right-hand side is checked against that operator. For a `Vec2`,
`velocity *= 0.98` is valid and `velocity += 2` is an error.

Complex left-hand sides such as calls and computed indexes are rejected.

## Boundaries

To keep malformed input from exhausting the compiler stack, analysis rejects
more than 64 nested constructs or 128 operators in one expression segment with
`TL0001`. Split deeply nested functions or long expressions into smaller pieces.
Comments and string contents do not count toward these limits.

Tua does not statically prove every Lua pattern. These remain gradual or
conservative:

- dynamic metatable construction, computed `__index` chains, and runtime
  `getmetatable` mutation;
- arbitrary mutation of table shapes over time;
- values stored through `_G` or unknown globals;
- runtime-computed `require` paths;
- advanced LuaDoc generics and complex annotation forms;
- package management and dependency installation;
- runtime validation after code starts running.

Annotations are most useful at module boundaries, constructors, returned
module tables, and LOVE-facing asset or object code. Highly dynamic glue can
remain ordinary Lua or use an explicit `any` boundary.

Tua is source-to-source tooling for Lua 5.1 syntax, LuaJIT, and LOVE 11.x. It
deliberately does not provide:

- a new Lua VM or a Lua 5.2+ output target;
- whole-program optimization or minification;
- execution of project code to discover types;
- a bundled Lua debugger, VM, or runtime profiler;
- an executable third-party plugin registry;
- completion that requires a network connection;
- full compatibility with other Lua language servers or typed Lua dialects.

Pre-1.0 releases follow the migration policy in [Releases](releases.md).
