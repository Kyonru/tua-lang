---
icon: lucide/home
---

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
- Plain `.lua` files and assets are copied into the build output so LOVE
  projects can produce runnable distribution folders.
- Built-in Lua and LOVE catalogs for completion, hover, signature help, and
  simple call checks.
- LOVE helpers such as `image "assets/player.png"`, `sound "assets/jump.wav"`,
  `music "assets/theme.ogg"`, `font("assets/ui.ttf", 18)`, `shader`,
  `imageData`, `canvas`, `quad`, `spriteBatch`, and `particles`, with static
  path validation for helpers and native LOVE constructors that load assets.
- Built-in `Vec2` and `vec2(x, y)` for common LOVE position/velocity code,
  backed by bundled BrineVector output.
- CLI commands for init, check, fmt, lint, build, watch, and LSP.
- A language server with diagnostics, hover, completion, signature help,
  document highlights, definition, references, rename, document symbols,
  workspace symbols, document links, code actions, code lens references, call
  hierarchy, inlay hints, full/range/delta semantic tokens, watched files, and
  workspace folders.
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
- [Learn](learn.md) - guided language path from Lua to typed Tua and LOVE.
- [Language](language.md) - overview, type surface, interop, LOVE helpers, and
  emission behavior.
- [Domain Plugins](plugins.md) - design boundaries for future LOVE-focused
  domain extensions.
- [Configuration](configuration.md) - `tua.toml` options and diagnostics.
- [CLI](cli.md) - command reference.
- [Language Server](lsp.md) - editor features and capability notes.
- [LSP Parity Matrix](lsp-parity.md) - Tua, LuaHelper, and rust-analyzer gap
  tracking.
- [LuaHelper Code Matrix](luahelper-code-matrix.md) - LuaHelper check,
  warning, annotation, and quick-fix parity inventory.
- [Lua Interop](lua-interop.md) - requiring Lua from Tua and Tua from Lua.
- [Examples](examples.md) - bundled projects and verification commands.
- [Roadmap](roadmap.md) - implementation status and planned parity work.
- [Architecture](architecture.md) - contributor guide, architecture map, and
  rust-analyzer gap matrix.
