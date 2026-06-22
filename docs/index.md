# Tua

**Typed Lua source-to-source tooling for LuaJIT and LOVE projects.**

Tua starts with Lua syntax, adds erased type syntax and a small amount of
LuaJIT-safe sugar, then emits readable Lua 5.1-compatible source. The generated
Lua has no Tua runtime dependency.

Tua is built for projects that want editor feedback and static checks without
leaving the normal Lua and LOVE workflow.

## What Tua Provides

- Erased local, function, alias, table-shape, optional-field, and union types.
- Lua 5.1-compatible output for LuaJIT and LOVE 11.x.
- `+=`, `-=`, `*=`, `/=`, and `%=` lowering for simple names and field chains.
- Project config with mirrored `src/**/*.tua` to `build/**/*.lua` output.
- CLI commands for init, check, build, watch, and LSP.
- A language server with diagnostics, hover, completion, signature help,
  document highlights, definition, references, rename, document symbols,
  workspace symbols, inlay hints, semantic tokens, watched files, and workspace
  folders.
- VS Code support for syntax highlighting, file icons, and LSP integration.

## Quick Start

```bash
cargo run -p tua-cli -- init
cargo run -p tua-cli -- check
cargo run -p tua-cli -- build
```

A minimal source file:

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

Emitted Lua:

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

## Documentation

- [Installation](installation.md) - local build, CLI, and editor setup.
- [Quickstart](quickstart.md) - create, check, build, and watch a project.
- [Language](language.md) - Tua syntax and emission behavior.
- [Configuration](configuration.md) - `tua.toml` options and diagnostics.
- [CLI](cli.md) - command reference.
- [Language Server](lsp.md) - editor features and capability notes.
- [Lua Interop](lua-interop.md) - requiring Lua from Tua and Tua from Lua.
- [Examples](examples.md) - bundled projects and verification commands.
- [Roadmap](roadmap.md) - implementation status and planned parity work.
