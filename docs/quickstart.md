---
icon: lucide/zap
---

# Quickstart

This guide creates a small Tua project, checks it, and emits Lua.

## Initialize

Create a project in the current directory:

```bash
cargo run -p tua-cli -- init
```

This creates:

```text
tua.toml
src/
```

The default config mirrors `src/**/*.tua` into `build/**/*.lua`.

## Add Source

Create `src/player.tua`:

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

return {
  makePlayer = makePlayer,
}
```

Create `src/main.tua`:

```tua
local Player = require("player")

local player = Player.makePlayer("Lua interop")
player.hp -= 7
```

## Check

```bash
cargo run -p tua-cli -- check
```

The checker reports syntax, type, config, and enabled lint diagnostics.

## Build

```bash
cargo run -p tua-cli -- build
```

This writes Lua output under `build/`, preserves ordinary `require(...)` calls,
lowers static `import(...)` bindings to runtime `require(...)` calls, and writes
optional `*.lua.map.json` line maps for traceback remapping when `emit_maps` is
enabled.

## Watch

```bash
cargo run -p tua-cli -- watch
```

The watcher runs an initial build, uses native filesystem notifications when
available, then debounces source, asset, and configuration changes before
rebuilding. It falls back to content-comparing polling when required, keeps
project analysis warm, and rechecks unchanged importers when a required
module's exported API changes.

## Try Bundled Examples

```bash
make build-features
make build-love-minimal
make build-love-full
```

When LOVE is installed:

```bash
make run-love-minimal
```
