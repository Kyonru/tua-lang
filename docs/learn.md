---
icon: lucide/book-open
---

# Learn Tua

This tutorial teaches the Tua syntax you need to write a small typed LOVE
project. You will add types to normal Lua, describe game data, split code into a
module, use LOVE callbacks, and see how classes work.

Tua is still Lua: tables, functions, `if`, loops, closures, metatables, and
`require(...)` keep their normal runtime behavior. Tua adds static information
for diagnostics and editor tooling, then emits readable Lua.

Before starting, follow the [Quickstart](quickstart.md) to initialize a project
and open `src/main.tua` in VS Code.

## 1. Type Values And Functions

In `src/main.tua`, start with ordinary Lua locals and add annotations after
their names. Function parameters and return values use the same syntax:

=== "Tua"

    ```tua
    local score: number = 0
    local title: string = "Tiny Arena"
    local running: boolean = true

    local function addScore(amount: number): number
      score += amount
      return score
    end

    print(title, running, addScore(10))
    ```

=== "Generated Lua"

    ```lua
    local score = 0
    local title = "Tiny Arena"
    local running = true

    local function addScore(amount)
      score = score + amount
      return score
    end

    print(title, running, addScore(10))
    ```

The annotations disappear from generated Lua. The checker still uses them to
reject calls such as `addScore("ten")`, and the editor uses them for hover,
completion, signature help, and inlay hints.

!!! tip "Type boundaries, not every local"
    Tua infers obvious local types, so `local score = 0` is a `number` without
    an annotation. Add explicit types at useful boundaries: function
    parameters, returns, module APIs, class fields, and important state tables.
    The primitive types are `number`, `string`, `boolean`, and `nil`. Use `any`
    as an explicit escape hatch for deliberately dynamic Lua.

## 2. Describe Game Data

Replace `src/main.tua` with the next example. Use `type` aliases to describe
table shapes; this step also introduces a literal string union, an optional
field, an inferred array, and nil narrowing:

=== "Tua"

    ```tua
    type Direction = "left" | "right"

    type Player = {
      name: string,
      x: number,
      hp: number,
      direction: Direction,
      nickname?: string,
    }

    local player: Player = {
      name = "Mina",
      x = 80,
      hp = 100,
      direction = "right",
    }

    local inventory = { "key", "potion" }
    table.insert(inventory, "map")

    local function label(value: string | nil): string
      if value ~= nil then
        return value
      end

      return "empty"
    end

    print(label(player.nickname), inventory[1])
    ```

=== "Generated Lua"

    ```lua
    local player = {
      name = "Mina",
      x = 80,
      hp = 100,
      direction = "right",
    }

    local inventory = { "key", "potion" }
    table.insert(inventory, "map")

    local function label(value)
      if value ~= nil then
        return value
      end

      return "empty"
    end

    print(label(player.nickname), inventory[1])
    ```

The important pieces are:

- `type Player = { ... }` describes a Lua table and emits no runtime code.
- `nickname?: string` means the field may be absent.
- `string | nil` means the value may be either type.
- checking `value ~= nil` narrows the value to `string` inside that branch.
- `"left" | "right"` limits a value to those exact strings and powers value
  completion.
- `{ "key", "potion" }` is inferred as `string[]`; array indexing and
  `ipairs(...)` retain the element type.

Table literals are still Lua tables. Tua checks their fields only when it knows
the expected shape.

## 3. Write A Module

Create `src/player.tua`. Keep the type private and return a normal Lua module
table:

=== "Tua"

    ```tua
    type Player = {
      name: string,
      x: number,
      hp: number,
    }

    local function new(name: string): Player
      return {
        name = name,
        x = 80,
        hp = 100,
      }
    end

    local function move(player: Player, distance: number)
      player.x += distance
    end

    return {
      new = new,
      move = move,
    }
    ```

=== "Generated Lua"

    ```lua
    local function new(name)
      return {
        name = name,
        x = 80,
        hp = 100,
      }
    end

    local function move(player, distance)
      player.x = player.x + distance
    end

    return {
      new = new,
      move = move,
    }
    ```

Tua indexes simple returned module tables. A caller gets completion for `new`
and `move`, the `new` result keeps its `Player` fields, and definitions and
rename work across the module boundary.

Use normal `require("player")` for package-root modules, third-party libraries,
and dynamic Lua loading. `import("./player")` is available for a static
source-relative child import and lowers to `require("player")`.

## 4. Connect The Module To LOVE

Replace `src/main.tua` with the module consumer and normal LOVE callbacks:

=== "Tua"

    ```tua
    local Players = require("player")
    local player = Players.new("Mina")

    function love.update(dt: number)
      Players.move(player, 120 * dt)
    end

    function love.draw()
      love.graphics.print(player.name, player.x, 80)
    end
    ```

=== "Generated Lua"

    ```lua
    local Players = require("player")
    local player = Players.new("Mina")

    function love.update(dt)
      Players.move(player, 120 * dt)
    end

    function love.draw()
      love.graphics.print(player.name, player.x, 80)
    end
    ```

The built-in LOVE catalog knows callback parameters and LOVE object methods.
It supplies completion, hover, signatures, and callback diagnostics without a
project-local LOVE definition file.

LOVE object methods normally use Lua's colon syntax, such as
`image:setFilter("nearest")`, because `:` passes the receiver as `self`. Tua
warns when a known LOVE method is called with an incorrect dot form.

Run **Tua: Run LOVE Project** from the command palette. The extension builds
the project, launches LOVE, and mirrors runtime output into both the task
terminal and the `tua` output channel.

## 5. Use Classes When They Fit

Modules and typed tables are enough for most Lua code. When state and behavior
belong to the same object, Tua classes provide typed syntax for the same
metatable pattern. Create `src/counter.tua` to try one:

=== "Tua"

    ```tua
    final class Counter {
      value: number
      history: number[]

      init(start: number) {
        self.value = start
        self.history = {}
      }

      add(amount: number): number {
        self.value += amount
        table.insert(self.history, self.value)
        return self.value
      }
    }

    local score = Counter.new(0)
    score:add(10)
    ```

=== "Generated Lua"

    ```lua
    local Counter = {}
    Counter.__index = Counter
    Counter.__type_id = 1
    Counter.__is = {
      [1] = true,
    }

    local function Counter_init(self, start)
      self.value = start
      self.history = {}
    end

    local function Counter_add(self, amount)
      self.value = self.value + amount
      table.insert(self.history, self.value)
      return self.value
    end
    Counter.add = Counter_add

    local function Counter_new(start)
      local self = setmetatable({}, Counter)
      Counter_init(self, start)
      return self
    end

    Counter.new = Counter_new

    local score = Counter.new(0)
    Counter_add(score, 10)
    ```

Declare every instance field before using it. Inside `init` and methods,
`self` has the class type, so `self.` completion includes fields and methods.
Initialize array fields such as `number[]` with `{}` when the constructor does
not receive their initial contents.

Classes are closed by default. Write `open class Entity` only when another
class needs to extend it; subclasses use `extends`, `override`, and
`super(...)`. See [Classes](language.md#classes) for inheritance and runtime
`is` checks.

## 6. Check, Build, And Inspect

VS Code reports syntax and type diagnostics while you edit. These commands are
the useful daily loop:

- **Tua: Explain Type / Why Is This Any?** shows where a selected type came
  from or where information became dynamic.
- **Tua: Open Generated Lua Beside Tua** opens the emitted file for the active
  saved source.
- **Tua: Select Inlay Hint Mode** controls inferred type and parameter hints.
- **Tua: Run LOVE Project** builds and runs the configured LOVE output.
- **Tua: Debug LOVE Project** builds and launches source-mapped Lua debugging,
  so breakpoints and stack frames return to `.tua` source.
- **Tua: Focus Output** opens language-server, build, and runtime logs.

With the standalone CLI installed, the equivalent terminal commands are:

```bash
tua check
tua build
tua watch
```

`check` reports diagnostics, `build` writes `.lua` files under the configured
output directory, and `watch` rebuilds after source, asset, or configuration
changes. Treat the output directory as generated code; edit files under `src/`.

## Next Steps

- [Language](language.md) is the complete syntax and type reference.
- [Lua Interop](lua-interop.md) covers `.lua` modules and LuaDoc annotations.
- [Configuration](configuration.md) documents project paths, diagnostics,
  formatting, LuaJIT guidance, and editor settings.
- [Input](input.md), [State Stores](stores.md), [State Machines](state-machines.md),
  [Entity Component Systems](ecs.md), and [Object Pooling](object-pooling.md)
  cover optional game-focused runtime domains.
- [Examples](examples.md) lists runnable feature and LOVE projects.
