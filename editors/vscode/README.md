# Tua for VS Code

Typed Lua editing and LOVE2D tooling for Tua projects.

Tua is a typed Lua superset that emits readable Lua 5.1-compatible code for
LuaJIT and LOVE 11.x. This extension brings compiler-backed editing, LOVE-aware
completions, asset tooling, project graphs, and build/run commands into VS
Code without changing the normal Lua and LOVE workflow.

## Why This Extension

- Compiler-backed Tua editing: completion, hover, signature help, definitions,
  references, rename, diagnostics, inlay hints, semantic tokens, and workspace
  symbols come from the same Tua semantic pipeline as the CLI.
- LOVE-aware authoring: callback snippets, callback signature checking, LOVE
  API documentation and overloads, colon-call guidance, and config-aware
  `love.conf(t)` completion are built in.
- Better game-project tooling: sprite sheets, SignalBus events, assets, module
  dependencies, ECS, and object pools all have editor support instead of living
  as untyped strings and conventions.
- Generated Lua visibility: inspect emitted Lua beside the current Tua file and
  jump between Tua and generated Lua with source maps.
- Project-oriented workflow: initialize a project, run LOVE from VS Code tasks,
  inspect language-server health, and keep `tua.toml` fully documented in the
  editor.

## Quick Start

1. Install the extension from the
   [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=tua-lang.tua),
   [Open VSX](https://open-vsx.org/extension/tua-lang/tua), or a
   [GitHub release VSIX](https://github.com/Kyonru/tua-lang/releases).
2. Run `Tua: Initialize Tua Project`.
3. Open `src/main.tua`.
4. Run `Tua: Run LOVE Project`.

Example:

```tua
type Player = {
  x: number,
  y: number,
  speed: number,
}

local player: Player = {
  x = 160,
  y = 120,
  speed = 90,
}

function love.update(dt: number)
  if love.keyboard.isDown("right") then
    player.x += player.speed * dt
  end
end

function love.draw()
  love.graphics.print("Hello from Tua", player.x, player.y)
end
```

From there you can:

- use normal completion and hover on LOVE APIs;
- rebuild and open emitted Lua with `Tua: Open Generated Lua Beside Tua`;
- jump between source and output with `Tua: Jump Between Tua and Generated Lua`;
- run and restart the LOVE project without leaving VS Code.

## What You Get

### Core Tua Editing

- TextMate and semantic highlighting for Tua, Lua interop, LOVE-aware helpers,
  event names, annotations, types, functions, methods, parameters, properties,
  labels, and literals.
- Zero-delay quick suggestions for Tua files using VS Code's normal completion
  flow.
- Hover, completion, signature help, definition, references, rename, document
  symbols, workspace symbols, document highlight, call hierarchy, code lenses,
  and range/full/delta semantic tokens.
- Inlay hint modes: Full, Compact, Types Only, Parameters Only, and Off.
- `Tua: Explain Type / Why Is This Any?` for tracing annotations, inference,
  widening, narrowing, module resolution, and dynamic fallback points.
- Source actions for organizing imports, sorting imports, removing unused
  imports, fixing safe diagnostics, and other compiler-backed refactors.
- `tua.toml` completion, hover documentation, enum/default value completion,
  and unknown-key quick fixes.

### LOVE2D Workflow

- LOVE callback snippet completion for `love.load`, `love.update(dt)`,
  `love.draw`, `love.conf(t)`, touch/gamepad callbacks, file drop callbacks,
  and more.
- Callback signature checking and typo diagnostics for misspelled or
  incompatible LOVE callback declarations.
- LOVE API completion with overloads, documentation, and parameter details.
- Asset-path understanding for both Tua helpers and native LOVE constructors
  that load files.
- Baton-backed `input({...})` completion and checking for LOVE keys,
  scancodes, mouse buttons, gamepad axes, buttons, hats, controls, and pairs.
- `Tua: Run LOVE Project` and `Tua: Restart LOVE Project` as cancellable VS Code
  tasks. Runtime `print(...)`, stdout, and stderr are mirrored into both the
  task terminal and the `tua` output channel, and emitted Lua traceback lines
  are mapped back to original Tua files when source maps are enabled.
- `Tua: Debug LOVE Project` bridges generated Lua to the optional
  [Second Local Lua Debugger](https://marketplace.visualstudio.com/items?itemName=ismoh-games.second-local-lua-debugger-vscode)
  extension. Breakpoints stay in `.tua`; stepping, variables, watches,
  conditional breakpoints, and evaluation remain standard Lua debugging. Game
  workspaces can also opt a `lua-local` launch configuration into the same
  preparation pipeline with `tuaProject: true`.
- Compiler-backed sprite sheet inspection and preview through
  `spriteSheet({...})` metadata. Preview is currently gated by
  `tua.preview.enabled`.

### Game-Focused Project Tooling

- Asset Explorer grouped by asset kind and static usage, with reference
  navigation, rename, reveal, preview, and conservative unused-asset hints.
- SignalBus event-name completion and a compiler-backed signal graph with
  typed payload display, conflict inspection, and source navigation.
- Module dependency explorer for `require(...)` and `import(...)` relationships,
  unresolved modules, duplicate resolutions, cycles, reverse dependencies, and
  inclusion paths.
- ECS explorer showing components, systems, queries, and conservative read/write
  access insights.
- Object pool explorer showing declarations plus visible `acquire`, `release`,
  `clear`, and `prewarm` lifecycle operations.

## Key Commands

### Project and Runtime

- `Tua: Initialize Tua Project`
- `Tua: Run LOVE Project`
- `Tua: Debug LOVE Project`
- `Tua: Restart LOVE Project`
- `Tua: Focus Output`
- `Tua: Open Generated Lua Beside Tua`
- `Tua: Jump Between Tua and Generated Lua`

### Editing and Insight

- `Tua: Explain Type / Why Is This Any?`
- `Tua: Select Inlay Hint Mode`
- `Tua: Toggle Experimental Completions`
- `Tua: Copy Fully Qualified Symbol Name`
- `Tua: Workspace Symbols by Module`
- `Tua: Organize Imports`
- `Tua: Sort Imports`
- `Tua: Remove Unused Imports`
- `Tua: Fix All Safe Diagnostics`

### Project Explorers

- `Tua: Refresh Asset Explorer`
- `Tua: Preview Sprite Animations`
- `Tua: Show Signal Graph`
- `Tua: Show Module Dependency Graph`
- `Tua: Show ECS Explorer`
- `Tua: Show Object Pool Explorer`
- `Tua: Why Is This Module Included?`

### Server and Support

- `Tua: Restart Language Server`
- `Tua: Focus Output`
- `Tua: Open Settings`
- `Tua: Select Server`
- `Tua: Show Server Health`
- `Tua: Copy Debug Info`
- `Tua: Copy Cache Stats`
- `Tua: Save Cache Stats`
- `Tua: Reset Completion Learning Stats`
- `Tua: Save CPU Profile`
- `Tua: Save Heap Summary`
- `Tua: Report Issue`

## Configuration

Important VS Code settings:

- `tua.server.path`: path to the `tua` executable used to launch the language
  server.
- `tua.love.command`: path to the LOVE executable used by
  `Tua: Run LOVE Project`.
- `tua.trace.server`: LSP trace level (`off`, `messages`, `verbose`).
- `tua.log.verbosity`: server log verbosity (`off`, `error`, `info`, `debug`,
  `trace`).
- `tua.preview.enabled`: enables experimental editor previews such as sprite
  animation preview.
- `tua.completion.sequenceRanking.enabled` and
  `tua.completion.multiToken.enabled`: experimental completion lanes controlled
  together by `Tua: Toggle Experimental Completions`.

Project behavior still comes primarily from `tua.toml`, and the extension
attaches to that file directly for configuration authoring.

Published VSIX packages bundle the matching platform server binary and do not
download a server during activation. If you want to use a workspace-local
binary such as `target/debug/tua`, select it with `Tua: Select Server`. VS Code
workspace trust is required before workspace-local binaries are offered or
executed.

## Docs

- [Project docs](https://kyonru.github.io/tua-lang)
- [Installation](https://kyonru.github.io/tua-lang/installation/)
- [Language server details](https://kyonru.github.io/tua-lang/lsp/)
- [Configuration reference](https://kyonru.github.io/tua-lang/configuration/)
- [Language overview](https://kyonru.github.io/tua-lang/language/)
- [Examples](https://kyonru.github.io/tua-lang/examples/)

## Privacy and Support

The extension does not send telemetry or upload project source code.

`Tua: Copy Debug Info` gathers extension version, VS Code version, platform,
workspace trust, resolved server path, server version, server state, workspace
directory, and recent startup failure text when available. It does not copy
source file contents.

`Tua: Report Issue` opens the project issue URL with the same diagnostic bundle
pre-filled for review before submission.
