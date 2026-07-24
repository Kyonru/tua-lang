---
icon: lucide/zap
---

# Quickstart

This is the shortest path from an empty folder to a running Tua project in VS
Code. The extension includes the Tua language server, so this workflow does not
require Rust, Cargo, or a separate Tua installation.

## 1. Initialize A Project

1. Install the [Tua VS Code extension](installation.md#recommended-vs-code-extension).
2. Open a project folder in VS Code.
3. Open the command palette and run **Tua: Initialize Tua Project**.
4. Open `src/main.tua`.

Initialization creates the minimum project structure without overwriting files
that already exist:

```text
tua.toml
src/
src/main.tua
```

`tua.toml` maps `src/**/*.tua` to `build/**/*.lua`. Its `main = "main.tua"`
setting also emits `src/main.tua` as `build/main.lua`, which is the entrypoint
LOVE expects.

## 2. Write Tua

Replace `src/main.tua` with this small LOVE program:

=== "Tua"

    ```tua
    local x: number = 80
    local speed: number = 120

    function love.update(dt: number)
      x += speed * dt
    end

    function love.draw()
      love.graphics.circle("fill", x, 120, 24)
    end
    ```

=== "Generated Lua"

    ```lua
    local x = 80
    local speed = 120

    function love.update(dt)
      x = x + speed * dt
    end

    function love.draw()
      love.graphics.circle("fill", x, 120, 24)
    end
    ```

Tua is Lua with gradual type checking and editor feedback. Type annotations are
removed during build, conveniences such as `+=` lower to portable Lua, and the
generated files have no Tua runtime dependency.

## 3. Run The Project

With LOVE 11.x installed, run **Tua: Run LOVE Project**. The extension builds
the project, launches LOVE from `build/`, and mirrors `print(...)`, stdout, and
stderr into both the task terminal and the `tua` output channel.

Use **Tua: Restart LOVE Project** after changes. If LOVE is not found, set
`tua.love.command` to the LOVE executable path in VS Code settings.

To debug instead of only run, use **Tua: Debug LOVE Project**. See
[Debugging](debugging.md) for debugger installation, source maps, generated-Lua
inspection, and launch guidance.

## 4. Use The Editor

Normal VS Code editing is the primary Tua workflow. The extension provides
diagnostics, formatting, completion, hover, signature help, navigation,
rename, semantic highlighting, and inlay hints for `.tua` files.

These commands are useful immediately:

- **Tua: Open Generated Lua Beside Tua** saves and rebuilds the active source,
  then shows its freshly emitted Lua beside it.
- **Tua: Debug LOVE Project** builds and starts a source-mapped Lua debug
  session for the project.
- **Tua: Explain Type / Why Is This Any?** traces where the selected value's
  type came from or where static information was lost.
- **Tua: Select Inlay Hint Mode** switches between full, compact, type-only,
  parameter-only, and disabled hints.
- **Tua: Toggle Experimental Completions** enables or disables contextual
  sequence ranking and multi-token suggestions for the current workspace.
- **Tua: Focus Output** opens server, build, and runtime logs.

!!! info "Experimental completions are opt-in"
    Experimental completion features are off by default. The toggle changes
    only their two top-level feature gates; advanced and future
    model-integration policy settings remain untouched.

## 5. Know The Project Model

- Write project source under `src/`. Files ending in `.tua` are checked and
  emitted as Lua; ordinary `.lua` modules can live beside them for interop.
- Use normal Lua `require("module")` calls. Tua resolves simple project modules
  for completion, navigation, diagnostics, and exported types.
- Add annotations where they provide value. Unannotated or highly dynamic Lua
  remains valid and falls back to `any` when Tua cannot prove a type.
- Edit `tua.toml` to change source/output paths, formatting, diagnostics, LuaJIT
  guidance, or ECS behavior. VS Code completes and validates its settings.

!!! warning "Do not edit generated Lua"
    Treat `build/` as generated output. Edit `.tua` source rather than emitted
    `.lua` files.

## Terminal Commands

If you also install the [standalone Tua CLI](installation.md#standalone-cli-release),
the equivalent terminal workflow is:

```bash
tua check
tua build
tua watch
```

`check` reports diagnostics without writing output, `build` emits Lua, and
`watch` rebuilds when project files change. The server bundled inside the VS
Code extension is used by extension commands but is not added to your shell
`PATH`.

Continue with [Learn Tua](learn.md) for the language model, or open the
[configuration reference](configuration.md) and
[LOVE-focused language guide](language.md#love-focused-layer) when you need
project-specific details.
