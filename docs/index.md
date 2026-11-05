---
icon: lucide/home
---

# Tua

**Typed Lua source-to-source tooling for LuaJIT and LOVE projects.**

Tua is a gradually typed Lua language and toolchain. You write familiar Lua
with erased type annotations and a small amount of LuaJIT-safe syntax, get
compiler-backed editor feedback, and build readable Lua 5.1-compatible source
for LuaJIT or LOVE 11.x.

Tua works alongside ordinary `.lua` modules, so an existing project can adopt
`.tua` files gradually instead of changing runtime, framework, or module
system.

```mermaid
flowchart LR
  Source[Typed .tua source] --> Tua[Tua checks and builds]
  Lua[Existing .lua modules] --> Tua
  Tua --> Output[Readable .lua output]
  Output --> Runtime[LuaJIT or LOVE 11.x]
```

!!! tip "Fastest way to try Tua"
    Install the [Tua VS Code extension](installation.md), open a folder, and
    run **Tua: Initialize Tua Project**. Published extension packages include
    the matching language server, so this workflow does not require Rust,
    Cargo, or a separate Tua installation. Continue with the
    [Quickstart](quickstart.md) to build and run the project.

## Start Here

| Your goal | Read this |
| --- | --- |
| Install Tua and choose an editor or CLI setup | [Installation](installation.md) |
| Create and run a first project in VS Code | [Quickstart](quickstart.md) |
| Debug generated LOVE code from `.tua` source | [Debugging](debugging.md) |
| Learn Lua values, tables, loops, and method calls | [Lua Basics](lua-basics.md) |
| Learn the language by building a small LOVE project | [Learn Tua](learn.md) |
| Add Tua to an existing Lua codebase | [Lua Interoperability](lua-interop.md) and [Configuration](configuration.md) |
| Model data and share type-safe module APIs | [Typing](typing.md) and [Modules And Imports](modules.md) |
| Import classes, annotate instances, and use inheritance | [Classes](classes.md) |
| Write API comments and understand annotations | [Comments And Documentation](comments.md) |
| Fix missing suggestions, formatting, imports, or run failures | [Troubleshooting](troubleshooting.md) |
| Ship a game to players | [Packaging Games](packaging.md) |
| Generate typed modules from source assets | [Asset Pipelines](asset-pipelines.md) |
| Look up syntax or a specific feature | [Language Guide](language.md) |
| Animate sprites from a sprite sheet | [Sprites And Animation](sprites.md) |
| Move things with vectors, interpolation, and colors | [Vectors And Game Math](vectors.md) |
| Decouple game systems with typed events | [Signals](signals.md) |
| Define typed keyboard, mouse, or gamepad controls | [Input](input.md) |
| Build a managed LÖVE audio graph | [Audio Systems](audio.md) |
| Look up commands, build output, and diagnostics | [CLI](cli.md) and [Diagnostic Codes](luahelper-code-matrix.md) |

## From Tua To Lua

Types improve checking and editor tooling, then disappear from generated code.
Tua conveniences such as compound assignment lower to portable Lua:

<div class="tua-code-compare" markdown="1">
<div class="tua-code-compare__pane" markdown="1">
<p class="tua-code-compare__label">Tua</p>

```tua
type Player = {
  x: number,
  speed: number,
}

local player: Player = {
  x = 80,
  speed = 120,
}

function love.update(dt: number)
  player.x += player.speed * dt
end
```

</div>
<div class="tua-code-compare__pane" markdown="1">
<p class="tua-code-compare__label">Generated Lua</p>

```lua
local player = {
  x = 80,
  speed = 120,
}

function love.update(dt)
  player.x = player.x + player.speed * dt
end
```

</div>
</div>

!!! info "Runtime model"
    Ordinary typed code has no Tua runtime dependency. Opt-in helpers such as
    vectors, sprite sheets, stores, state machines, ECS, and object pools can emit
    their required vendored Lua modules into the build output when used.

## What Tua Gives You

- **Gradual types for Lua.** Add annotations to locals, functions, tables,
  arrays, modules, and classes where static feedback is valuable; leave
  dynamic code as normal Lua or use `any` explicitly.
- **Compiler-backed editing.** The VS Code extension provides diagnostics,
  completion, hover, signature help, navigation, rename, semantic highlighting,
  inlay hints, formatting, generated-Lua inspection, and project explorers.
- **A practical build toolchain.** `tua check`, `tua build`, and `tua watch`
  check `.tua`, emit `.lua`, copy ordinary Lua and assets, and preserve source
  information for traceback mapping and Lua debugger integration.
- **LOVE-aware development.** Built-in LOVE APIs, callbacks, object methods,
  enums, asset paths, sprites, vectors, Signals, state stores, state machines,
  ECS, and object pools receive dedicated checking and editor support.
- **Normal Lua interoperability.** Use `require(...)`, keep `.lua` and `.tua`
  files in one project, and describe Lua module APIs with supported LuaDoc
  annotations.

Tua does not introduce a VM, bytecode format, package manager, or replacement
runtime. Generated files are ordinary Lua and can be inspected, debugged, and
distributed with the rest of the project.

## Choose A Game Feature

Use ordinary LOVE APIs whenever they fit. These optional helpers add typed
configuration and editor support, and builds include their Lua runtimes when used.

| Task | Guide |
| --- | --- |
| Animate a sprite sheet | [Sprites And Animation](sprites.md) |
| Move and interpolate values | [Vectors And Game Math](vectors.md), [Tweening](tweening.md) |
| Follow a player or convert screen coordinates | [Camera](camera.md) |
| Send typed events between systems | [Signals](signals.md) |
| Share observable state | [State Stores](stores.md) |
| Model states and transitions | [State Machines](state-machines.md) |
| Map keyboard, mouse, and gamepad controls | [Input](input.md) |
| Process entities by component | [ECS](ecs.md) |
| Reuse frequently allocated objects | [Object Pooling](object-pooling.md) |
| Manage sound effects, buses, and music | [Audio Systems](audio.md) |

## Tooling References

Use [Configuration](configuration.md) for `tua.toml`, [CLI](cli.md) for commands,
and [Language Server](lsp.md) for editor features and settings. Formatting and
lint rules have separate guides: [Formatter](formatter.md) and [Linter](linter.md).
[Hot Reload](hot-reload.md) explains which edits can preserve running state.
[Releases](releases.md) covers checksums and compatibility.

## Project Information

- [Architecture](architecture.md) explains crate ownership, shared analysis,
  request and build flows, and where changes belong.
- [External API And MCP](external-api-bridge.md) documents the shipped,
  read-only stdio JSON-RPC API and MCP adapter for trusted local tools.
- [Contributing](https://github.com/Kyonru/tua-lang/blob/main/CONTRIBUTING.md),
  [Security](https://github.com/Kyonru/tua-lang/blob/main/SECURITY.md), and
  [Support](https://github.com/Kyonru/tua-lang/blob/main/SUPPORT.md) document
  project participation, private vulnerability reporting, and help channels.

Source, issues, and releases are available in the
[Tua GitHub repository](https://github.com/Kyonru/tua-lang).
