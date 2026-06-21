# tua v1 Specification

tua is a typed Lua superset that compiles to plain Lua 5.1-compatible source for
LuaJIT and LÖVE projects. The type system is erased at compile time: type
annotations help catch mistakes before running the game, but they do not create a
new runtime or change the emitted Lua data model.

The v1 goal is a small, practical toolchain for game code:

- write `.tua` files with gradual static types;
- emit readable `.lua` files that can run in LÖVE without a tua runtime;
- preserve normal Lua `require` module patterns;
- provide compiler diagnostics and editor feedback early;
- avoid language features that require migrating away from LuaJIT.

Luau is an important reference point for typed Lua design, but tua's niche is
different: tua is source-to-source tooling for existing LuaJIT/LÖVE projects,
not a replacement VM.

## Runtime Target

Generated Lua must be compatible with Lua 5.1-style Lua as used by LuaJIT and
LÖVE 11.x. The emitter must avoid Lua 5.2+ runtime syntax and semantics.

tua v1 does not require a runtime support library. If a `.tua` program compiles
successfully, the emitted `.lua` should be understandable as ordinary Lua.

## Source And Output Layout

The default project layout is:

```text
.
├── src/
│   └── main.tua
├── build/
│   └── main.lua
└── tua.toml
```

By default, `tua build` mirrors `src/**/*.tua` into `build/**/*.lua`.
Non-`.tua` files needed by a LÖVE project may be copied by future tooling, but
v1 only specifies compilation of `.tua` sources.

Default `tua.toml`:

```toml
source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = true
```

## Language Surface

tua starts from Lua syntax and adds erased type syntax plus a small amount of
LuaJIT-safe sugar.

### Primitive Types

v1 supports these primitive type names:

- `number`
- `string`
- `boolean`
- `nil`
- `any`

`any` is the gradual escape hatch. Values of type `any` may be assigned to or
from any other type without a type error.

### Type Aliases

Type aliases define names for types and are erased from emitted Lua.

```tua
type Vec2 = {
  x: number,
  y: number,
}
```

Emitted Lua:

```lua
```

No code is emitted for a type alias.

### Table Shapes

Table shape types describe known fields.

```tua
type Player = {
  pos: Vec2,
  hp: number,
  alive: boolean,
}
```

An object-like table literal can be checked against a table shape when an
annotation gives the compiler enough context.

```tua
local player: Player = {
  pos = { x = 10, y = 20 },
  hp = 100,
  alive = true,
}
```

### Optional Fields

A field followed by `?` is optional.

```tua
type SaveSlot = {
  name?: string,
  level: number,
}
```

An optional field may be absent or may contain a value compatible with the
declared field type. In v1, optional fields are treated as `T | nil` when read.

### Union Types

Union types use `|`.

```tua
type MaybeNumber = number | nil
```

Unions are checked structurally: assigning a value to a union is allowed when the
value is compatible with at least one member of the union.

### Function Annotations

Function parameters and returns may be annotated.

```tua
local function damage(player: Player, amount: number): Player
  player.hp -= amount

  if player.hp <= 0 then
    player.alive = false
  end

  return player
end
```

Emitted Lua:

```lua
local function damage(player, amount)
  player.hp = player.hp - amount

  if player.hp <= 0 then
    player.alive = false
  end

  return player
end
```

Return annotations are checked against explicit `return` statements when the
compiler can determine the returned expression type.

### Local Annotations

Local declarations may include type annotations.

```tua
local score: number = 0
local title: string = "Arena"
local current: Player | nil = nil
```

Emitted Lua:

```lua
local score = 0
local title = "Arena"
local current = nil
```

## Compound Assignment

tua v1 supports these compound assignment operators:

- `+=`
- `-=`
- `*=`
- `/=`
- `%=`

Compound assignment is lowered to normal Lua assignment.

```tua
local x = 10
x += 5
player.hp -= amount
```

Emitted Lua:

```lua
local x = 10
x = x + 5
player.hp = player.hp - amount
```

v1 only supports compound assignment when the left-hand side is:

- a simple local/global name, such as `x`;
- a plain field chain, such as `player.hp` or `world.player.hp`.

The compiler must reject complex left-hand sides where lowering could evaluate
side effects more than once.

Rejected examples:

```tua
getPlayer().hp -= amount
players[i()].hp -= amount
```

## Type Checking Model

tua v1 is gradual and explicit-first.

The checker must:

- check annotated function parameters and returns;
- check annotated local declarations;
- check assignments to known typed variables and table fields;
- infer primitive literal types for `number`, `string`, `boolean`, and `nil`;
- infer simple table literals when an annotation supplies the expected shape;
- allow `any` to flow to and from all types;
- preserve permissive behavior for unannotated dynamic Lua code.

The checker should produce diagnostics for obvious mismatches:

```tua
local hp: number = "full"
```

Expected diagnostic:

```text
type mismatch: expected number, found string
```

Unannotated code defaults to permissive checking:

```tua
local value = getSomethingDynamic()
value.anything.deeply.dynamic = true
```

This should not fail v1 type checking unless later annotations constrain the
value.

## Compiler Behavior

The compiler pipeline is:

1. Read project config.
2. Discover `.tua` source files under the configured source directory.
3. Parse each file.
4. Build per-file and project-wide type information.
5. Type-check annotated code.
6. Lower tua-only syntax to Lua syntax.
7. Erase type aliases and type annotations.
8. Emit Lua 5.1-compatible `.lua` files under the configured output directory.
9. Optionally emit source maps or line maps when `emit_maps = true`.

Emitted Lua should be readable and line-stable where practical. Readability is
more important than aggressive minification or optimization.

### Error Handling

By default, `tua build` must fail when type errors are present.

`tua build --emit-on-error` should still emit Lua for files that can be parsed
and lowered, while reporting diagnostics and returning a non-zero exit code.

Parse errors always block emission for the affected file.

## Modules

tua v1 preserves Lua's existing `require` style.

```tua
local Player = require("player")
```

Emitted Lua:

```lua
local Player = require("player")
```

v1 does not introduce `import`, `export`, or type-only import syntax. Cross-file
type discovery may be added by scanning project files for type aliases, but
runtime module semantics remain normal Lua.

## CLI

The `tua` binary provides the project workflow.

### `tua init`

Creates a default `tua.toml` and starter `src/` directory if they do not exist.

### `tua check`

Parses and type-checks the project without writing `.lua` output.

Expected behavior:

- returns exit code `0` when no errors are found;
- returns non-zero when parse or type errors are found;
- prints diagnostics with file, line, column, severity, and message.

### `tua build`

Checks and emits Lua output.

Expected behavior:

- mirrors `src/**/*.tua` to `build/**/*.lua`;
- erases type-only syntax;
- lowers supported sugar;
- preserves `require`;
- fails without emission when blocking errors are found.

### `tua build --emit-on-error`

Reports diagnostics but still emits Lua for files that can be parsed and lowered.
The command should return a non-zero exit code if any error was reported.

### `tua watch`

Watches project sources and rebuilds when `.tua` files change.

Expected behavior:

- perform an initial build;
- debounce rapid filesystem events;
- re-check and re-emit changed files;
- keep running until interrupted.

### `tua lsp`

Starts the tua language server over standard input/output using the Language
Server Protocol.

The LSP server should reuse the same parser, checker, and diagnostic formatting
as the CLI.

## Editor Tooling

v1 includes a minimal VS Code extension.

The extension should provide:

- language id: `tua`;
- file association: `*.tua`;
- basic syntax highlighting for Lua plus tua type syntax;
- language server launch through `tua lsp`;
- diagnostics from the language server;
- hover for known type aliases, local variables, function signatures, and table
  fields;
- basic completion for known type aliases and table fields.
- signature help for known function calls.
- document highlights for known symbol usages.
- incremental document sync with open, change, save, close, and watched-file
  update handling.
- workspace folder initialization and workspace folder change notifications.

The extension may assume `tua` is available on `PATH`, with a setting for an
explicit server binary path.

## Diagnostics

Diagnostics must be implementation-oriented and stable enough for CLI and LSP
consumers.

Each diagnostic should include:

- file path;
- line;
- column;
- severity;
- message;
- optional diagnostic code.

Example:

```text
src/player.tua:12:15 error TL1002: type mismatch: expected number, found string
```

Suggested diagnostic code ranges:

- `TL0001`-`TL0999`: parse and syntax errors;
- `TL1000`-`TL1999`: type errors;
- `TL2000`-`TL2999`: lowering and emission errors;
- `TL3000`-`TL3999`: project configuration errors.

## Non-Goals For v1

tua v1 does not include:

- runtime type checks;
- a custom runtime library;
- a new VM;
- new module syntax;
- type-only imports or exports;
- generics;
- metatable typing;
- vararg typing;
- advanced inference;
- package management;
- optimization beyond straightforward syntax lowering;
- Lua 5.2+ output targets;
- perfect modeling of dynamic keys or shape-changing tables.

## Reference Example

Input `src/player.tua`:

```tua
type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  pos: Vec2,
  hp: number,
  alive: boolean,
  name?: string,
}

local function damage(player: Player, amount: number): Player
  player.hp -= amount

  if player.hp <= 0 then
    player.alive = false
  end

  return player
end

return {
  damage = damage,
}
```

Output `build/player.lua`:

```lua
local function damage(player, amount)
  player.hp = player.hp - amount

  if player.hp <= 0 then
    player.alive = false
  end

  return player
end

return {
  damage = damage,
}
```

## Acceptance Criteria

The v1 implementation is acceptable when:

- `.tua` source files compile to Lua 5.1-compatible `.lua` files;
- emitted Lua can run in LÖVE without tua being present at runtime;
- annotated primitive, function, union, optional field, and table-shape examples
  are checked;
- compound assignment examples lower correctly;
- unsupported complex compound assignments produce diagnostics;
- `require` calls are preserved;
- `tua check`, `tua build`, `tua watch`, and `tua lsp` exist;
- the VS Code extension can surface diagnostics from `tua lsp`.
