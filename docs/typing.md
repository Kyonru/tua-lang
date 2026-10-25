---
icon: lucide/binary
---

# Typing In Tua

Tua is gradually typed Lua. Type information improves diagnostics, completion,
hover, navigation, rename, signature help, and inlay hints, then disappears from
the generated Lua. You can type a whole subsystem, annotate only its public
boundaries, or deliberately leave dynamic values as `any`.

This guide explains Tua's type system as a whole. See the
[Language Guide](language.md) for the complete syntax reference and
[Lua Interoperability](lua-interop.md) for describing existing `.lua` code with
LuaDoc.

## Start With Boundaries

Tua infers straightforward locals, so useful annotations usually belong on
function parameters and returns, shared state, class fields, callbacks, and
module APIs:

```tua
type Player = {
  name: string,
  health: number,
}

local function damage(player: Player, amount: number): number
  player.health -= amount
  return player.health
end

local player: Player = {
  name = "Mina",
  health = 100,
}
```

The emitted Lua contains the same tables and functions without the annotations
or alias declaration. Types do not add runtime wrappers or change table layout.

## Type Forms

### Primitives And `any`

The primitive types are `number`, `string`, `boolean`, and `nil`:

```tua
local score: number = 0
local title: string = "Arena"
local active: boolean = true
local missing: nil = nil
```

`any` is the gradual escape hatch. Values can be assigned to and from `any`, so
use it at intentionally dynamic boundaries rather than as a default:

```tua
local plugin: any = require("third_party_plugin")
plugin.configure(dynamicOptions)
```

An `any` value gives up guarantees downstream. When an unexpected `any` appears
in VS Code, run **Tua: Explain Type / Why Is This Any?** to trace its source.

### Aliases And Table Shapes

A type alias gives a reusable name to a type and emits no runtime code:

```tua
type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  position: Vec2,
  health: number,
  nickname?: string,
}
```

Table compatibility is structural: a value satisfies `Player` when its
required fields have compatible types. Optional fields may be absent, but must
have the declared type when present.

Inside an alias, `Self` refers to that alias. This is useful for recursive data:

```tua
type Node = {
  name: string,
  parent: Self | nil,
}
```

### Arrays, Dictionaries, And LuaDoc Tuples

Use `T[]` for an array and `{ [Key]: Value }` for a dictionary:

```tua
local checkpoints: Vec2[] = {}
local scores: { [string]: number } = {}
local spawn: { x: number, y: number } = { x = 80, y = 120 }
```

Native Tua annotations do not currently have a fixed-length positional table
type. In Lua interop annotations, LuaCATS tuple syntax remains supported, for
example `---@alias Spawn [number, number]`. Prefer a named record when the
positions have meaning, or `number[]` when the length is intentionally dynamic.

An empty table is a valid initializer when an array or dictionary annotation
provides its element types. Array element types flow through supported indexed
reads and writes, `table.insert`, `ipairs`, and common loop patterns.

Dictionary reads (`scores[key]` and `scores.name`) return `Value | nil`, because
a key may be absent. Store the result in a local and guard it, or provide a
default with `or`, before using it as `Value`. Values yielded by `pairs` retain
the declared value type. This is stricter than earlier releases.

Ordinary `values[index]` reads use the declared element type, matching the
dense-array contract. When the index may be outside the populated range, use
Lua's `rawget(values, index)`: Tua infers `T | nil`, and direct indexed
comparisons with `nil` remain valid runtime bounds checks.

Parenthesize a union or function type before making it an array:

```tua
local values: (number | string)[] = {}
local callbacks: ((dt: number) -> nil)[] = {}
```

### Unions, Nilable Values, And Literal Choices

A union accepts a value compatible with any member:

```tua
type Identifier = number | string
local target: Player | nil = nil
```

Literal string unions model a closed set of choices and power string-value
completion:

```tua
type Facing = "left" | "right"
local facing: Facing = "left"
```

For a longer named list, an erased `enum` is equivalent to a literal union:

```tua
enum MovementState
  "idle"
  "run"
  "jump"
  "roll"
end
```

### Function Types

Function declarations use annotations after parameter names and after the
parameter list:

```tua
local function move(dx: number, dy: number): boolean
  return dx ~= 0 or dy ~= 0
end
```

Callback types use `(parameters) -> returns`:

```tua
type Predicate = (player: Player) -> boolean
type Decoder = (text: string) -> (boolean, string)
```

The parenthesized return list in `Decoder` represents multiple Lua return
values. Parameter names are retained for editor presentation but do not affect
compatibility.

Trailing parameters can be optional or have a default:

```tua
local function label(player: Player, prefix?: string): string
  return (prefix or "Player") .. ": " .. player.name
end

local function heal(player: Player, amount: number = 5)
  player.health += amount
end
```

Use `...: Type` in a declaration and `...Type` in a function type for typed
varargs:

```tua
local function logAll(prefix: string, ...: string)
  print(prefix, ...)
end

type Logger = (prefix: string, ...string) -> nil
```

Generics are not currently part of Tua. Use arrays, dictionaries, concrete
aliases, and `any` at genuinely dynamic boundaries.

## Inference And Assignment

An unannotated local with a statically known initializer gets a stable, widened
type for that lexical binding:

```tua
local score = 10       -- number
score = 12             -- accepted
score = "high"         -- error: expected number

local state = "idle"   -- string, not the exact literal "idle"
state = "running"      -- accepted
```

The binding keeps that contract across branches and loops. A shadowing `local`
creates a new binding rather than changing the outer one.

Lua initializes a declaration without a value to `nil`, so declare intended
optional state explicitly:

```tua
local pending: number | nil = nil
pending = 10
```

Use an annotation when an empty table, `nil`, or another broad initializer does
not contain enough information to express the intended contract.

When a value fails its contract somewhere inside a table, dictionary, tuple,
or callback, `TL1003` names the path to the failing part and its two leaf
types instead of printing both complete outer types:

```tua
type Enemy = { name: string, stats: { hp: number } }

local boss: Enemy = { name = "Hydra", stats = { hp = "lots" } }
-- TL1003: type mismatch at stats.hp: expected number, found string
```

For a table literal, the diagnostic points at the deepest source value Tua can
identify, here `"lots"`.

## Narrowing

Nil checks narrow optional values inside the guarded branch:

```tua
local function playerName(player: Player | nil): string
  if player ~= nil then
    return player.name
  end

  return "none"
end
```

A plain truthiness check narrows the same way, and both sides of `and` are
narrowed together:

```tua
local voice = audio:play("engine")
if voice then
  voice:fadeTo(0.8, 0.5)
end

if player and target then
  aim(player.position, target.position)
end
```

Short-circuit guards also narrow the right operand: `target ~= nil and
target.x > 0` accesses `x` only when `target` exists. With `target == nil or
target.x > 0`, the right operand is checked under the opposite condition.
This applies to expressions in assignments as well as `if` conditions.

A truthy value can still be `true`, so truthiness removes `nil` but keeps
`boolean`. The `else` branch keeps the original type.

An early return also narrows the remaining statements. Return inference keeps
that information in returned records, including functions exported by modules:

```tua
local function position(x: number | nil)
  if not x then
    return nil
  end
  return { x = x } -- inferred as { x: number }
end

local target = position(10)
if target then
  print(target.x + 1)
end
```

Use `value is Type` when Tua can emit an equivalent runtime identity check:

```tua
local function stringify(value: string | number): string
  if value is string then
    return value
  end

  return "" .. value
end
```

Primitive `is` checks lower to Lua `type(...)` comparisons. Visible metatable
classes and native Tua classes use their supported runtime identity mechanisms.
Erased structural aliases, arrays, dictionaries, and arbitrary table shapes
cannot be tested at runtime and are rejected as `is` targets.

Narrowing is branch-local. It does not permanently change the declared or
inferred type after control flow rejoins.

## Sharing Types Between Modules

Declare a public alias with `export type`. Keep declaration-only contracts in a
`.d.tua` module and consume them with a named type-only import:

```tua title="src/types.d.tua"
export type Player = {
  name: string,
  health: number,
}

export type Room = {
  name: string,
  spawn: { x: number, y: number },
}

type InternalScratch = { id: number }
```

```tua title="src/world.tua"
import type { Player, Room as LevelRoom } from "./types"

local current: LevelRoom | nil = nil

local function enter(player: Player, room: LevelRoom)
  current = room
end
```

Named imports are the concise default for types used frequently in a file. Use
a namespace when several modules export the same names or when keeping the
owner visible improves a large API:

```tua
import type Shared from "./types"
local current: Shared.Room | nil = nil
```

In VS Code, exported project aliases appear in type completion before they are
imported. Accepting one inserts or extends an `import type` declaration. If an
unknown type already exists in the file, use the **Import type** quick fix.
Existing named and namespace imports are reused, including imports written with
an equivalent module specifier.

Build public facades with type re-exports:

```tua title="src/game_types.d.tua"
export type { Player, Room as LevelRoom } from "./types"
```

Only aliases marked `export type` cross a Tua module boundary. This rule is the
same whether the consumer uses `import type` or reaches the module through a
runtime `require` or `import` binding. Imported names cannot collide with local
type aliases or runtime declarations.

### Declaration-Only `.d.tua` Files

Use `.d.tua` for modules that describe types but have no runtime value:

- they are parsed, checked, indexed, watched, and available to editor features;
- they support exported aliases, type imports, enums, and type re-exports;
- runtime statements are rejected;
- `tua build` emits no `.lua` file or source map for them;
- type-only edges do not introduce runtime module loads or runtime require
  cycles.

A regular `.tua` file may also export types alongside runtime code. Its types
are erased while its runtime statements are emitted normally. Choose `.d.tua`
when the entire module is a declaration surface.

Type re-export cycles receive a dedicated diagnostic. Recursive structural
types can still refer across declaration modules; recursion is kept bounded
during checking and display.

## Structural And Nominal Types

Aliases for table shapes are structural. Independently declared shapes with
compatible required fields can be assigned to one another.

Native Tua classes are nominal across module boundaries. Two unrelated classes
do not become compatible merely because their fields happen to match. Class
fields, constructor parameters, method parameters, and returns use the same
type forms documented above. See [Classes](language.md#classes) for inheritance,
`Self`, `super`, and override rules.

Tua also recognizes a conservative subset of common Lua metatable patterns.
Prefer explicit aliases or native classes when a stable public contract matters;
dynamic metatable construction may intentionally fall back to `any`.

## Checking And Editor Feedback

Run the project checker without emitting Lua:

```sh
tua check
```

Type errors are always reported by `tua check` and block `tua build` by default.
The linter is a separate layer for configurable code-quality rules; see
[CLI](cli.md) and [Linter](linter.md).

The VS Code extension uses the same semantic model as the checker:

- hover shows resolved and inferred types;
- completion uses table fields, module exports, literals, and narrowed values;
- go-to-definition follows aliases and type-only imports;
- signature help presents typed parameters and returns;
- inlay hints expose inferred local, parameter, and return types;
- **Tua: Explain Type / Why Is This Any?** traces inference and gradual
  boundaries.

Type names are completed only in type positions. Type-only imports create no
runtime local and therefore do not appear as runtime values.

## Practical Guidelines

1. Annotate API boundaries and important state before annotating every local.
2. Use literal unions for closed string choices and `T | nil` for optional
   state.
3. Give empty arrays, dictionaries, and initially nil values an explicit type.
4. Keep related public contracts in a `.d.tua` declaration module.
5. Prefer named type imports for frequently used contracts; use a namespace
   when provenance or name disambiguation is useful.
6. Confine `any` to the smallest dynamic boundary and translate it into typed
   data early.
7. Use a runtime validator when untrusted data enters the program. Erased types
   do not validate files, network payloads, or user input at runtime.

## Current Boundaries

Known primitive operands are checked for arithmetic, ordering, concatenation,
negation, and length operations (`TL1003`). Narrow unions before applying an
operator that is not valid for every member; use `tonumber` for string-to-number
conversion. Unknown values and `any` retain gradual behavior.

Tua intentionally remains gradual and does not try to prove arbitrary Lua
behavior. Expect conservative results around dynamically computed table keys,
runtime-generated modules, `_G`, complex metatable mutation, and unknown
third-party values. Generics and runtime checks for erased structural types are
not currently supported.

For a plain `.lua` boundary, add supported LuaDoc annotations or a configured
declaration-only type library. Those mechanisms are documented in
[Lua Interoperability](lua-interop.md) and
[Configuration](configuration.md#declaration-only-type-libraries).
