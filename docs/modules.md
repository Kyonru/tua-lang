---
icon: lucide/folders
---

# Modules And Imports

A runtime module returns a Lua value, usually a table of functions or a class.
A type export describes values without loading code. Keep these two roles clear
when splitting a project into files.

## Choose An Import

| You need | Write | Runtime effect |
| --- | --- | --- |
| A Lua or Tua module | `local Player = require("player")` | Loads the module through Lua's `require`. |
| A statically checked project import | `local Player = import("./player")` | Lowers to `require` using the resolved project path. |
| An exported type alias | `import type { Player } from "./types"` | Erased; loads nothing. |
| An exported protocol | `import type { Updatable } from "./contracts"` | Erased; loads nothing. |
| A group of type aliases | `import type Types from "./types"` | Erased; use names such as `Types.Player` in type positions. |

Use runtime `import(...)` for supported static top-level local bindings. It is
not JavaScript's `import` syntax. Keep ordinary or dynamic Lua loading as
`require(...)`. Tua does not install third-party packages; provide their runtime
Lua files yourself. See [Lua Interop](lua-interop.md) for existing libraries and
declaration-only type libraries.

## Build A Two-File Module

With the default `source = "src"`, create these files:

```tua title="src/player.tua"
export type Player = {
  name: string,
  health: number,
}

local function new(name: string): Player
  return { name = name, health = 100 }
end

local function damage(player: Player, amount: number)
  player.health = math.max(0, player.health - amount)
end

return { new = new, damage = damage }
```

```tua title="src/main.tua"
import type { Player } from "./player"
local Players = require("player")

local player: Player = Players.new("Mina")
Players.damage(player, 25)
assert(player.health == 75)
```

Run `tua check` and `tua build` from the folder containing `tua.toml`.
The output contains `build/player.lua` and `build/main.lua`. The `Player` alias
and its type import disappear; the runtime require and function calls remain.
An annotation is useful for a public contract, but `Players.new("Mina")` can
also carry an inferred result type.

This example uses no LOVE APIs. With Lua installed, run it from the output
folder so `require("player")` finds the generated module:

```bash
cd build
lua main.lua
```

You can use `luajit main.lua` instead when LuaJIT is installed. The assertion
passes without output. Tua compiles `.tua` files; the Lua/LOVE runtime executes
the result. For a test suite, use [native unit testing](testing.md): `tua test` builds the
project and runs `_spec` files with Busted. Plain assertion scripts remain useful
for small checks.

## Resolve Paths From The Project

For a default project, `src/combat/enemy.tua` is required as `"combat.enemy"`,
without `src/` or the `.tua` extension. From `src/combat/system.tua`, a static
`import("./enemy")` resolves relative to that source file. The output module
path is still based on the configured source root.

Tua's static import syntax supports source-relative child imports; do not
assume arbitrary filesystem traversal or a path beginning with `../` is valid.
Use a project module path when you need another part of the source tree.
Module filenames and paths must also work on the player's filesystem; keep
capitalization consistent.

Assets under the source tree are copied to corresponding output paths. A path
such as `"assets/player.png"` is a runtime asset path, not a `require` specifier.
See [Configuration](configuration.md#root-options) for custom roots and
[LOVE asset loaders](language.md#asset-loaders) for asset behavior.

## Share Types Without Loading Code

Use a `.d.tua` file when the whole module is a type contract:

```tua title="src/events.d.tua"
export type Damage = { target: string, amount: number }
export type Box<T> = { value: T }
```

Only explicitly exported aliases and protocols are public. A protocol can
also be declared next to runtime classes using `export protocol Name { ... }`.
`.d.tua` files do not emit Lua; do not use a runtime `require` for a
declaration-only module. Named imports, renaming imports, namespace imports,
and type re-exports are covered in
[Sharing Types](typing.md#sharing-types-between-modules).
Generic aliases use the same export and import rules. For example,
`import type { Box } from "./events"` permits `Box<number>` in a type
annotation; namespace imports use `Events.Box<number>`. Type arguments and the
alias declaration are erased and add no runtime module load.
To share a generic class, export a generic instance alias beside the runtime
class value, then import that alias as a type and require the module for the
constructor:

```tua title="src/box.tua"
final class Box<T> {
  value: T
  init(value: T) { self.value = value }
  get(): T { return self.value }
}

export type BoxInstance<T> = Box<T>
return { Box = Box }
```

```tua title="src/main.tua"
import type { BoxInstance } from "./box"
local BoxModule = require("box")

local box = BoxModule.Box.new(42)
local typed: BoxInstance<number> = box
local value: number = typed:get()
```

The type import and generic arguments are erased. The runtime `require`
provides the constructor; project type metadata carries the original class
identity and substitutes imported fields and methods. Named, namespace, and
re-exported aliases are supported. Generic class constructors may use one
protocol bound per type parameter; imported and re-exported protocol identity is
preserved across the module boundary.

For a method contract shared by runtime modules, export the protocol from a
regular `.tua` module and import it only where the class needs its type:

```tua title="src/contracts.tua"
export protocol Updatable {
  update(dt: number): nil
}
```

```tua title="src/particle.tua"
import type { Updatable } from "./contracts"

final class Particle implements Updatable {
  update(dt: number): nil {
  }
}
```

The type-only import does not load `contracts` at runtime. A class can also
implement a protocol declared in its own source file.

A class constructor is a runtime value; an instance annotation is a type.
Export an alias such as `export type DialogueInstance = Dialogue` and import
it with `import type`. The complete two-file example is in
[Classes](classes.md#import-a-constructor-and-its-instance-type). The editor's
**Export instance type** action and type auto-import completion help create
these declarations.

## Understand Loading And Reloading

Lua normally caches module results in `package.loaded`. Requiring a module
again does not construct a fresh instance or rerun its initialization. Return
a factory when callers need independent objects.

Avoid runtime import cycles: initialization may observe a dependency before
its API is ready. Type-only imports do not load modules and can separate a
shared contract from initialization. Use **Tua: Show Module Dependency Graph**
to inspect static dependencies and **Tua: Why Is This Module Included?** to
trace a path to the active file.

Hot reload has additional rules for exported tables, captured functions,
class instances, and local state. Read [Hot Reload](hot-reload.md) before relying
on state surviving an edit.

## When Types Become Dynamic

Static returned tables and known function signatures are easiest to analyze.
Computed export keys, runtime mutation, and unresolved third-party APIs may
fall back to `any`. Prefer explicit return annotations or supported LuaDoc at
the boundary rather than adding annotations to every caller.

Run **Tua: Explain Type / Why Is This Any?** on the affected expression. For a
missing module, check the configured source root, path spelling, runtime files,
and [Troubleshooting](troubleshooting.md#an-import-is-missing-or-has-type-any).
