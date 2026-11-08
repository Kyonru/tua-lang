---
icon: lucide/book-open
---

# Lua Basics For Tua

Tua uses Lua's runtime rules. If you come from TypeScript, Swift, or another
language, learn these rules before adding types. You can run the examples in a
Tua project; type annotations are removed when it builds.

## Values And Locals

Use `local` for variables and functions that belong to the current scope.
Assigning an undeclared name writes a global, which can make dependencies hard
to see. Use explicit modules to share code.

```tua
local score = 0
local name = "Mina"
local active = true
local selected: string | nil = nil

score = score + 10
selected = name
```

`nil` represents absence. Only `false` and `nil` are false in a condition;
`0` and `""` are true. `and` and `or` return operands, not necessarily booleans.
For example, `value or fallback` replaces both `nil` and `false`. If `false`
is a meaningful value, check `value == nil` explicitly.

Tua infers a stable type from a known initializer. Use an annotation when a
variable starts as `nil` but will later hold a value, or when an empty table
needs an element type. See [Inference And Assignment](typing.md#inference-and-assignment).

## Tables, Arrays, And Dictionaries

Lua uses tables for records, sequences, and dictionaries:

```tua
local player = { name = "Mina", health = 100 }
local names: string[] = { "Mina", "Rin" }
local scores: { [string]: number } = { Mina = 10, Rin = 20 }

print(player.name, names[1], scores["Rin"])
table.insert(names, "Sol")
```

Sequence indexing starts at **1**. Use `#names` for a contiguous sequence's
length; it is not a dictionary entry count. Avoid holes in a sequence: `#`
is not a reliable count of elements when keys are missing, and `ipairs` stops
at the first absent element. For vectors, use `vector.length`, not `#vector`.

`player.name` is shorthand for `player["name"]`. Reading a missing key returns
`nil`; assigning `nil` to a key removes it. Types do not add runtime bounds or
missing-key checks, so validate external keys and indices yourself.

Assignment shares a table reference; it does not copy its fields:

```tua
local original = { score = 1 }
local same = original
same.score = 2
assert(original.score == 2)
```

Use an explicit copy when you need independent state. Nested tables need their
own copies too. `==` normally compares table identity, not field-by-field equality.

## Control Flow

Lua blocks close with `end`. Numeric `for` loops include their final bound:

```tua
for i = 1, 3 do
  print(i)
end

local names = { "Mina", "Rin" }
for index, name in ipairs(names) do
  if name ~= "" then
    print(index, name)
  end
end

local scores = { Mina = 10, Rin = 20 }
for name, score in pairs(scores) do
  print(name, score)
end
```

Use `ipairs` for a contiguous sequence and `pairs` for table entries; `pairs`
does not promise an iteration order. `break` exits a loop. Tua has no `continue`
statement; put the work inside a conditional instead.

## Functions And Multiple Returns

Functions can return several values without creating a table:

```tua
local function dimensions(): (number, number)
  return 320, 180
end

local width, height = dimensions()
local first = dimensions()
assert(width == first and height == 180)
```

A single assignment keeps the first return. A final function call in an
assignment, argument list, or table constructor can expand to multiple values;
a call in another position contributes one value. Parenthesizing the call also
reduces it to one value. Missing assignment values become `nil`.

Functions are values and can capture locals. A captured local remains available
after its declaring function returns. See [Function Types](typing.md#function-types)
to describe callbacks.

## Dot And Colon Calls

`object:method(value)` passes `object` as the first argument, conventionally
named `self`. `object.method(value)` does not.

```tua
local text = love.graphics.newText(love.graphics.getFont(), "Hello")
local width = text:getWidth()
```

Use colon calls for LOVE object methods and native Tua class instance methods.
Module functions such as `love.graphics.print(...)` generally use dots. Keep
the receiver when passing a method as a callback:

```tua
local measure = function(): number
  return text:getWidth()
end
```

## Common Syntax Differences

| Intent | Lua/Tua syntax |
| --- | --- |
| Not equal | `a ~= b` |
| Logical operators | `and`, `or`, `not` |
| Join strings | `"Score: " .. tostring(score)` |
| Optional value type | `string \| nil` |
| Optional record field | `{ nickname?: string }` |
| Array type | `number[]` |
| Function type | `(value: number) -> number` |
| Return a module API | `return { update = update }` |
| Import a runtime module | `local Game = require("game")` |

Native Tua class bodies use braces, but their nested Lua functions and
control-flow blocks still use `end`. Tua uses `protocol` for explicit,
method-only class contracts; it does not support JavaScript imports or
`interface`, `async`, optional chaining, or Luau casts. Generic type parameters
are available on type aliases (`type Box<T> = ...`); generic functions and
classes are not supported.

Continue with [Learn Tua](learn.md), [Modules And Imports](modules.md), or
[Typing In Tua](typing.md). Types improve static feedback; they do not validate
saved games, network data, or user input at runtime.
