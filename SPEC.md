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

By default, `tua build` mirrors the configured source tree into `build/`:

- `src/**/*.tua` is checked, lowered, erased, and emitted as matching
  `build/**/*.lua`;
- every non-`.tua` file is copied unchanged to the matching output path.

This makes `build/` a runnable Lua/LÖVE distribution directory containing code,
images, audio, fonts, maps, and other project files. Plain Lua files remain
interoperability files, not typed source files, unless a later feature adds
explicit Lua annotation checking.

If `source` and `out` point to the same directory, `tua build` runs in-place:
only `.tua` files are processed, non-`.tua` files are left alone, and emitted Lua
is written next to the source file. In-place builds are allowed but should be
used carefully because `main.tua` emits `main.lua`.

If a `.tua` file and a `.lua` file would both write the same output path, the
compiler emits a warning. In a separate output directory, generated Lua wins and
the colliding `.lua` asset copy is skipped. In an in-place build, the warning
reports that the sibling `.lua` file will be overwritten by generated Lua.

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
- string literal types such as `"static"` and `"stream"`
- built-in `Vec2` for common LOVE position and velocity values

`any` is the gradual escape hatch. Values of type `any` may be assigned to or
from any other type without a type error.

### Built-In Vec2

`Vec2` is a built-in table-shaped type:

```tua
local position: Vec2 = vec2(100, 200)
local x: number = position.x
local y: number = position.y
```

The constructor has signature `function vec2(x: number, y: number): Vec2`.
`position.x` and `position.y` complete and hover as `number`.

Emitted Lua uses a cached BrineVector constructor in v1:

```lua
local __tua_vec2 = require("brinevector")
local position = __tua_vec2(100, 200)
local x = position.x
local y = position.y
```

`tua build` writes a bundled `brinevector.lua` runtime into the output directory
when emitted Lua needs it, unless the project already provides a
`brinevector.lua` file. BrineVector is designed for LOVE/LuaJIT: it uses FFI
when JIT is available and falls back to standard Lua tables where that is the
better runtime choice.

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
type LoadMode = "static" | "stream"
```

Unions are checked structurally: assigning a value to a union is allowed when the
value is compatible with at least one member of the union.

### Enums

Enums are erased sugar for a string literal union. They exist to make long mode
lists easier to read and maintain, especially for LOVE APIs.

```tua
enum FilterMode
  "linear"
  "nearest"
end
```

This is equivalent to:

```tua
type FilterMode = "linear" | "nearest"
```

Only string literal enum members are supported in v1. Enum declarations emit no
runtime code. In LuaCATS annotation mode, an enum emits as an `---@alias` literal
union.

### Practical Metatable Typing

Tua supports a conservative static subset of common metatable patterns. When
both the base table and metatable are visible, `setmetatable(base, meta)` may
propagate method and operator facts into the base table shape for checking,
hover, completion, go to definition, and inferred expression types.

Supported v1 facts:

- function-valued table fields may be called as `obj.method(obj, ...)` or
  `obj:method(...)`;
- colon-defined methods with annotations, such as
  `function Account:withdraw(amount: number): number`, are typed internally as
  if they had a leading `self` parameter. If a type alias named `Account`
  exists, that receiver is typed as `Account`; otherwise it remains `any`;
- simple `__index` table/function fallbacks may expose known string keys;
- self-index class tables such as `Account.__index = Account` may expose
  methods on instances created with `setmetatable({}, Account)`;
- static prototype chains such as `setmetatable(Sub, { __index = Base })` may
  expose inherited fields and methods through later instances when each link is
  visible;
- constructor methods that visibly return `setmetatable(instance, self)`, such
  as `Account:new(...)`, may infer an Account-like instance result;
- `__newindex` may check simple assignment contracts when either
  `__newindex = function(t, key: "label", value: string) ... end` or
  `__newindex = { label = string }` is visible. Unknown keys remain
  permissive;
- visible arithmetic metamethods `__add`, `__sub`, `__mul`, and `__div` may
  provide operator result types;
- visible comparison metamethods `__eq`, `__lt`, and `__le` may provide
  boolean comparison result types;
- visible `__len` may provide the result type for `#value`;
- visible `__call` may make a table callable, with the table receiver treated
  as the hidden first metamethod argument;
- visible `__pairs` and `__ipairs` functions may type generic loop variables
  when their return type is a tuple-like key/value pair, for example
  `---@return [string, number]`;
- `tostring(value)` is treated as returning `string`.

Dynamic metatable mutation, computed `__index` chains, runtime iterator
triples, and unknown `setmetatable` inputs remain gradual.

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
- infer simple record-like and array-like table literals;
- infer bracket indexing for known arrays and string-keyed table fields;
- infer simple numeric, `ipairs`, and `pairs` loop variable types when the
  iterated value is statically known;
- use visible `__pairs` and `__ipairs` tuple return facts for generic loop
  variables when a metatable is statically known;
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
2. Discover project files under the configured source directory.
3. Parse each `.tua` file.
4. Build per-file and project-wide type information for parsed `.tua` files.
5. Type-check annotated code.
6. Lower tua-only syntax to Lua syntax.
7. Erase type aliases and type annotations.
8. Emit Lua 5.1-compatible `.lua` files under the configured output directory.
9. Copy non-`.tua` files unchanged into the configured output directory.
10. Optionally emit source maps or line maps when `emit_maps = true`.

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

## LOVE-Focused Builtins

tua includes a built-in LOVE API catalog for editor tooling and type-aware
checks. The catalog is available by default for completions, hover, signature
help, and simple call diagnostics.

### Asset Loader Helpers

tua provides typed LOVE-focused asset loader helpers:

```tua
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
```

These helpers are syntax sugar for ordinary LOVE calls and are lowered during
emission.

Emitted Lua:

```lua
local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")
```

`image` returns `Image`; `sound` returns `Source`. The checker should validate
obvious missing or unsupported asset paths when the project file exists on disk
and should stay permissive when a path cannot be proven statically.

### LOVE Object Method Calls

LOVE object methods usually expect Lua's colon-call syntax so `self` is passed
implicitly.

Preferred:

```tua
player:setFilter("nearest")
local width = player:getWidth()
```

Discouraged:

```tua
player.setFilter("nearest")
```

When the receiver is known to be a LOVE object and the target field is a known
LOVE method, the checker should warn on dot-style calls. The diagnostic should
point at the method name and suggest the equivalent colon call.

Completions for LOVE object methods should also prefer colon-call snippets. If
completion is requested after a dot, such as `player.setF`, accepting the method
completion may replace the dot with a colon and insert a call snippet.

## CLI

The `tua` binary provides the project workflow.

### `tua init`

Creates a default `tua.toml` and starter `src/` directory if they do not exist.

### `tua check`

Parses and type-checks the project without writing `.lua` output.

Expected behavior:

- returns exit code `0` when no errors are found, even if warnings are printed;
- returns non-zero when parse or type errors are found;
- prints diagnostics with file, line, column, severity, and message.

### `tua build`

Checks and emits Lua output.

Expected behavior:

- mirrors `src/**/*.tua` to `build/**/*.lua`;
- copies non-`.tua` files unchanged to matching output paths;
- supports in-place transpilation when `source` and `out` are the same
  directory;
- warns when a `.tua` output collides with an existing `.lua` file;
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

v1 includes a VS Code extension and a stdio LSP server.

The extension should provide:

- language id: `tua`;
- file association: `*.tua`;
- basic syntax highlighting for Lua plus tua type syntax;
- language server launch through `tua lsp`;
- diagnostics from the language server;
- hover for known aliases, locals, parameters, functions, table fields, Lua
  builtins, LOVE API items, module exports, and supported LuaDoc docs;
- completion for known aliases, locals, table fields, Lua builtins, LOVE API
  items, LuaDoc tags/types/snippets, require paths, required-module fields, and
  auto-import candidates;
- signature help for known function calls, including supported parameter docs;
- go to definition, references, rename, document links, document symbols,
  workspace symbols, document highlights, code actions, call hierarchy, inlay
  hints, and semantic tokens;
- incremental document sync with open, change, save, close, and watched-file
  update handling.
- workspace folder initialization and workspace folder change notifications.

The extension should use a bundled server binary when packaged, fall back to
`tua` on `PATH`, and allow an explicit server binary path through settings or
`TUA_SERVER_PATH` during development.

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
- `TL3000`-`TL3999`: project configuration errors;
- `TL4000`-`TL4999`: lint and style guidance diagnostics;
- `TL5000`-`TL5999`: built-in domain plugin diagnostics.

LuaJIT-oriented guidance uses the lint/style range and is disabled by default.
The initial codes are:

- `TL4032`: variable value type drift in opt-in JIT guidance mode;
- `TL4033`: known table-shape or field-type drift in opt-in JIT guidance mode;
- `TL4034`: dynamic table-key writes inside loops in opt-in JIT guidance mode.

These diagnostics are advisory. They must not change type inference, lowering,
emitted Lua, or runtime behavior.

Built-in domain plugin diagnostics use the plugin range. The initial ECS plugin
codes are:

- `TL5002`: ECS component value does not match a declared component schema;
- `TL5003`: ECS system/query component usage is invalid or unresolved.

Domain plugin diagnostics are opt-in through their project configuration and
must not execute project runtime code.

Inline diagnostic suppression is opt-in through project configuration. When
enabled, the compiler and language server recognize:

```lua
-- tua-ignore-next-line TL4018
player.setFilter("nearest")

local legacy: number = "soon" -- tua-ignore-line TL1003
```

`tua-ignore-line` targets diagnostics on the same line.
`tua-ignore-next-line` targets diagnostics on the following line.
`tua-ignore-file` may be enabled for migration-heavy files through
configuration, but should not be enabled by default.

Suppression comments may list exact codes such as `TL1003` or category groups
such as `TL1000`, `TL4000`, `type`, or `lint`. With no code, a suppression
matches all non-syntax diagnostics in its target scope. Invalid codes and
unused suppressions should produce warnings. Suppression comments must never
change type inference, lowering, emitted Lua, or runtime behavior.

## Non-Goals For v1

tua v1 does not include:

- runtime type checks;
- a custom runtime library;
- a new VM;
- new module syntax;
- type-only imports or exports;
- generics;
- advanced or dynamic metatable typing;
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
