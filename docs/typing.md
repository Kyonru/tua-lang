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

### Generic Type Aliases

Type aliases may declare required, unbounded type parameters. Supply every
argument explicitly when using the alias:

```tua
type Box<T> = { value: T }
type Result<T, E> = { value: T | nil, error: E | nil }

local box: Box<number> = { value = 4 }
local result: Result<string, string> = { value = "ready" }
```

Applications can be nested in records, arrays, unions, dictionaries, and
function types, and can be written in supported LuaDoc type annotations.
Generic alias arguments are substituted through the declared type shape; Tua
does not infer them from an initializer or function call. A
generic alias used without all its arguments, or with the wrong number of
arguments, reports `TL1004`. An unknown generic alias reports `TL1001`.
In VS Code, member completion and hover use the instantiated field types, and
Go to Definition on an imported generic alias navigates to its declaration.

Generic functions use declaration binders and infer type arguments from direct
call arguments. For example, `local function identity<T>(value: T): T return
value end` can be called with numbers or strings independently. The same
inference works for assigned function values, imported module functions, and
the corresponding unbounded LuaDoc form. Function bodies are checked while
their binders are symbolic. Inference that depends only on a return type or on
the parameter types of a callback is rejected with `TL1015`; generic methods,
explicit call-site type arguments, and bounded LuaDoc generic parameters are
not supported. Generic functions and aliases erase from emitted Lua.
To bound analysis work in generated source, Tua caches identical generic call
specializations for one source-file check and reports one `TL1015` after 4,096
distinct call instantiations. Splitting generated call-heavy code across files
keeps each check within that limit.

Do not write `identity<number>(1)` to select a type argument. Tua does not
support type arguments at call sites, and `<` / `>` already form valid Lua
comparison expressions, so this text is parsed as Lua expression syntax rather
than as a generic specialization. Let Tua infer the type from the argument, or
put the type on a variable or alias where type applications are supported.

When a generic parameter occurs inside a mutable array, dictionary, or record
field in the function signature, inference keeps it invariant across all call
arguments. For example, `overwrite(cats, animal)` is rejected when `cats` is a
`Cat[]` and `animal` is only an `Animal`; otherwise the function could write
that base value into the caller's narrower container. A read-only use such as
`first(cats)` still infers `Cat`. This rule applies to generic call inference;
existing assignment compatibility for non-generic arrays is unchanged.

Recursive generic aliases remain symbolic when resolution returns to the same
alias declaration, including when that recursive edge changes its type
arguments. For example, `Grow<T> = { next: Grow<T[]> }` resolves to one record
whose `next` field is `Grow<T[]>`; it does not expand into a long chain of
nested records. Separate depth and work limits still protect unusually deep
chains of distinct aliases. Full structural expansion is bounded by the number
of generic alias applications in the walk; member queries substitute the
requested path lazily so known fields keep their concrete types when the full
record expansion reaches that bound.

Generic class parameters are supported locally and across module boundaries
when a class exports a generic instance alias:

```tua
final class Pair<K, V> {
  first: K
  second: V

  init(first: K, second: V) {
    self.first = first
    self.second = second
  }
}

local pair = Pair.new(3, "ready")
local count: number = pair.first
local label: string = pair.second
```

The constructor infers each class parameter independently. Hover and completion
show the substituted member types, and class parameters are invariant because
instances are mutable. A bare generic class or an application with the wrong
number of arguments reports `TL1004`. Generic class types are erased from Lua;
the generated class remains an ordinary table and metatable. Local generic
inheritance supports applied parents such as `Child<U> extends Base<U>`;
non-generic subclasses can specialize a parent such as
`Score extends Base<number>`. Inherited fields, methods, `super(...)`
initializer calls, and base assignments keep those parent arguments. To share
a class, export `type BoxInstance<T> =
Box<T>` beside the runtime class value, then pair `import type` with a runtime
`require`; constructor inference and imported member types preserve the
producer's class identity. Generic class arguments are erased before Lua
emission.

### Protocol Contracts

Classes can opt in to a method-only [protocol contract](classes.md#protocol-contracts).
Unlike structural table aliases, a class must declare `implements` before its
instances can be assigned to that protocol type. Protocol-typed values expose
the declared methods; they do not expose extra fields from a concrete class or
support an implicit downcast. The words `protocol` and `implements` remain
available as ordinary Lua identifiers outside declaration headers, including
in modules that use Tua type annotations.

Generic functions and classes can use one protocol bound, such as
`T: Updatable`. The bound makes protocol methods available through `T` and
requires inferred or explicitly applied class arguments to come from a class
that declares `implements Updatable`. This is nominal conformance, not
structural matching. See [Generic Protocol Bounds](classes.md#generic-protocol-bounds).

When a generic function value is passed to a parameter with a known callback
signature, Tua specializes it from that expected parameter and return type. For
example, `identity<T>` can satisfy both `(number) -> number` and
`(string) -> string` contexts. This does not infer a generic callee's `T` from
an untyped lambda argument; that callback-only inference remains ambiguous.

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

Protocol arrays are mutable too, so assigning an existing concrete class array
to a protocol array would let writes through the protocol view put a different
conforming class into the original concrete array. Tua rejects that widening;
initialize the protocol view directly from its conformers instead:

```tua
local readable: Readable[] = { FileReader.new(), MemoryReader.new() }
```

This restriction applies to class-to-protocol array widening. Existing
class-to-base-class array compatibility is unchanged; use a fresh protocol
array literal when code needs a writable shared protocol collection.

Dictionary reads (`scores[key]` and `scores.name`) return `Value | nil`, because
a key may be absent. Store the result in a local and guard it, or provide a
default with `or`, before using it as `Value`. Values yielded by `pairs` retain
the declared value type. This is stricter than earlier releases.

An unannotated array of records infers one row type. A field that only some
rows define becomes optional, so every row can read it:

```tua
local perks = {
  { id = "laser", cooldown = 0.1 },
  { id = "ricochet", cooldown = 0, level = 1 }, -- level?: number
}
for _, perk in ipairs(perks) do
  if perk.level then
    perk.level += 1
  end
end
```

Rows that are not all plain records, such as records mixed with numbers or
class instances, keep a union of their types.

Ordinary `values[index]` reads use the declared element type, matching the
dense-array contract. When the index may be outside the populated range, use
Lua's `rawget(values, index)`: Tua infers `T | nil`, and direct indexed
comparisons with `nil` remain valid runtime bounds checks.

The built-in `table.remove(values[, index])` also returns `T | nil` for a
`T[]`: the array may be empty. Guard the result before using its members. This
type flows through inferred function returns, hover, completion, and checking;
`any` arrays and custom functions named `table.remove` remain dynamic or retain
their declared contracts.

For a typed `T[]`, `table.insert(values, value)` and
`table.insert(values, index, value)` check the inserted value against `T`.
The indexed form requires a number. Shadowed or replaced `table.insert`
functions keep their own contracts, and an explicitly `any` collection remains
dynamic. This check does not infer a contract for arbitrary dictionary shapes.

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

Anonymous functions and callbacks use the same parameter syntax:

```tua
local score = function(points: number = 10, bonus?: number): number
  return points + (bonus or 0)
end
print(score()) -- 10
```

Defaults run when the argument is `nil`, including omitted arguments. They are
applied on each call, can refer to surrounding locals, and work in nested
callbacks and single-line bodies. Optional parameters remain nullable inside
the body. Required parameters must precede optional/defaulted parameters.
References inside nested defaults participate in unused-local checking,
definition navigation and rename, with normal parameter shadowing.

`table.sort` infers both comparator parameters from the array element type:

```tua
local scores: { name: string, points: number }[] = {}
table.sort(scores, function(a, b)
  return a.points > b.points
end)
```

The callback gets member completion and hover, and its body must return a
boolean. Explicit parameter annotations are checked against the element type.
A locally shadowed or replaced `table.sort` keeps its own function contract.
For other typed callback arguments, unannotated parameters also inherit the
expected function type and callback bodies are checked against its return type.

Use `...: Type` in a declaration and `...Type` in a function type for typed
varargs:

```tua
local function logAll(prefix: string, ...: string)
  print(prefix, ...)
end

type Logger = (prefix: string, ...string) -> nil
```

The tail annotation is checked at calls too: `logAll("items", 1)` is an
error because each extra argument must be a string.

Generic type parameters are supported on aliases, free functions, and classes;
functions and class constructors infer binders from direct call arguments.
Generic classes can extend an applied generic parent. Protocols provide
explicit method contracts, and generic functions/classes can constrain a type
parameter to one protocol (`T: Updatable`). Generic alias binders also support
completion, definition, references, and rename within their declaration.
Arrays and dictionaries remain the built-in collection forms; use `any` at
genuinely dynamic boundaries.

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
creates a new binding rather than changing the outer one. Native annotations
and LuaDoc local contracts belong to that declaration: reusing its name in
another function or nested scope does not transfer the contract. This also
applies to captured locals and imported function return inference.

An unannotated function that reaches its end without returning a value has a
`nil` return type, including methods and anonymous callbacks. That keeps
fallthrough distinct from `any`: callers can see that no value is produced,
and a `nil` result cannot satisfy a non-nil return contract. A function that
returns a value-producing call whose result Tua cannot resolve remains
dynamic.

Lua initializes a declaration without a value to `nil`, so declare intended
optional state explicitly:

```tua
local pending: number | nil = nil
pending = 10
```

Lua also fills missing assignment slots with `nil`. Parentheses force a call
to produce one value; an unparenthesized final call can supply multiple values
in an assignment or argument list:

```tua
local function point(): (number, number) return 10, 20 end
local function draw(x: number, y: number) print(x, y) end

local x, y = point()   -- both number
local first, absent = (point()) -- absent is nil
draw(point())          -- equivalent to draw(10, 20)
-- draw((point()))     -- error: only one argument
```

When Tua cannot infer one expression in an explicit multi-value return list,
that slot remains dynamic. It does not erase the types or positions of the
other returned values.

Use an annotation when an empty table, `nil`, or another broad initializer does
not contain enough information to express the intended contract.

When a value fails its contract somewhere inside a table, dictionary, tuple,
or callback, `TL1003` names the path to the failing part and its two leaf
types instead of printing both complete outer types:

```tua
type Enemy = { name: string, stats: { hp: number } }

local boss: Enemy = { name = "Hydra", stats = { hp = "lots" } }
-- TL1003: type mismatch at boss.stats.hp: expected number, found string
```

For a table literal, the diagnostic points at the deepest source value Tua can
identify, here `"lots"`.

Assignment errors name the destination. When a call supplies the value, the
message also identifies its return position, for example:

```text
type mismatch at height: expected number, found nil (return value 2 of `size`)
```

The editor highlights the destination for call results, so separate failures in
one multi-value assignment remain distinguishable. Missing assignment values
are identified explicitly as Lua nil padding.

Function contract errors also identify the return slot and function name when
available, or the parameter whose default is invalid:

```text
type mismatch at return value 2 of `dimensions`: expected number, found string
type mismatch at default for parameter `speed`: expected number, found string
argument count mismatch: expected 2, found 1; missing required parameters: `height`
```

A missing return path explains that the function can finish without returning a
value. Check each reachable branch against the declared return contract. For
argument-count errors, supply the named required parameters or check the call
against its signature. A dot/colon hint is included only when the checker can
establish that the receiver is missing.

CLI diagnostics include the file, line, and column. In the editor, the Problems
entry navigates to the bad return expression, the parameter whose default
fails, or the call.

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

Plain record paths can also be guarded: after `if state.target then`,
`state.target.value` uses the non-nil target type. The same applies after
`assert(state.target)` and a nil-check guard that returns early. These refinements
are shared by checking, completion, hover and return inference. Calls, member
writes (including writes through aliases), root reassignment and closure capture
invalidate the path guard. Metatable-backed paths remain conservative; capture
and guard a local when you need a stable value across those operations.

A call to the builtin `assert(value)` also narrows a local for subsequent
statements, including completion, hover and inferred returns. Nil comparisons
such as `assert(value ~= nil)` work too. Reassigning the local ends the guard.
Tua does not use shadowed or replaced `assert` functions as guards, or assertions
with function calls inside their arguments, which could mutate the guarded value.

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

Checking the first value of a call also narrows the values returned with it,
when the function returns either `nil` alone or a first value that is never
`nil` or `false`:

```tua
local function project(z: number)
  if z <= 0 then
    return nil
  end
  return 320 / z, 240 / z
end

local x, y = project(depth)
if x then
  print(x + y) -- y: number
end
```

The narrowing ends when either local is reassigned, and it does not apply when
the first value may be `false` or is `any`.

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

export type Box<T> = { value: T }

type InternalScratch = { id: number }
```

```tua title="src/world.tua"
import type { Player, Room as LevelRoom, Box } from "./types"
import type Shared from "./types"

local current: LevelRoom | nil = nil
local box: Box<number> = { value = 4 }
local namespacedBox: Shared.Box<string> = { value = "Mina" }

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
export type { Player, Room as LevelRoom, Box } from "./types"
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
type forms documented above. See [Classes](classes.md) for inheritance,
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
