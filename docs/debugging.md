---
icon: lucide/bug
---

# Debugging

Tua debugging is Lua debugging with source maps. LOVE executes the generated
`.lua` files, while VS Code maps breakpoints and stopped stack frames back to
the original `.tua` source.

## Quick Setup

1. Install the [Tua VS Code extension](installation.md).
2. Install
   [Second Local Lua Debugger](https://marketplace.visualstudio.com/items?itemName=ismoh-games.second-local-lua-debugger-vscode).
3. Open the folder containing `tua.toml` and place a breakpoint in a `.tua`
   file.
4. Run **Tua: Debug LOVE Project** from the command palette, or add the launch
   configuration below and start it from the Run and Debug view.

Both workflows build the project before launching LOVE. If LOVE is not on
`PATH`, set `tua.love.command` to the executable path in VS Code settings.

Keep source-map emission enabled in `tua.toml`:

```toml
emit_maps = true
```

This is the default for new projects.

!!! important "Opt the launch configuration into Tua"
    A normal `lua-local` configuration bypasses Tua's build and source-map
    preparation. Set `tuaProject: true` as shown below, or use **Tua: Debug LOVE
    Project**, so Tua can prepare and restore the generated output correctly.

## Run From The VS Code UI

In your game's workspace, create or update `.vscode/launch.json`:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Debug Tua LOVE Game",
      "type": "lua-local",
      "request": "launch",
      "tuaProject": true,
      "program": {
        "command": "${config:tua.love.command}",
        "communication": "pipe"
      }
    }
  ]
}
```

Select **Debug Tua LOVE Game** in the Run and Debug view and press ++f5++. Tua
uses the workspace folder containing this launch configuration to find
`tua.toml`; no output paths need to be duplicated in `launch.json`.

Optional values supported by Second Local Lua Debugger, such as `stopOnEntry`,
`breakInCoroutines`, or extra `args`, can be added to the same configuration.
Tua prepends the generated game directory to `args` before LOVE starts.

## What Runs

The debug target is the configured output directory, normally `build/`. Tua
uses Second Local Lua Debugger's `lua-local` adapter for execution, stepping,
variables, watches, conditional breakpoints, and debug-console evaluation.

Tua owns the integration around that adapter:

1. Run `tua build` for the active project.
2. Add a temporary debugger bootstrap to the output `conf.lua`.
3. Launch LOVE against the generated output directory.
4. Map `.tua` breakpoints to generated Lua and map stopped frames back to Tua.
5. Restore the prepared output and maps when the session ends or fails to
   start.

Neither workflow adds debugger code to project source.

### Source-Map Files

With `emit_maps = true`, each generated Lua file can have two adjacent maps:

| File | Used for |
| --- | --- |
| `*.lua.map` | Standard source map v3 consumed by compatible Lua debuggers |
| `*.lua.map.json` | Tua traceback remapping and source/generated navigation |

Unmapped generated helper lines are skipped while stepping by default. Source
maps currently provide line provenance, so mapped target columns begin at
column zero.

!!! warning "Rebuild after source changes"
    Tua builds whenever the command or launch configuration starts. After
    editing source, stop the current session and start the configuration again.
    Restarting only the existing Lua debug session does not invoke a new Tua
    build.

## Inspect Generated Lua

Use **Tua: Open Generated Lua Beside Tua** to inspect the emitted file for the
active saved source. Use **Tua: Jump Between Tua and Generated Lua** to move
between mapped lines in either direction.

This is useful when a stop lands near lowered class, helper, or compound
assignment code and you need to see the exact Lua executed by LOVE.

## Troubleshooting

| Problem | Check |
| --- | --- |
| Second Local Lua Debugger is unavailable or disabled by the environment | Install and enable it in the same VS Code build and profile as Tua. |
| LOVE cannot be launched | Set `tua.love.command` to the LOVE executable and trust the workspace. |
| A `.tua` breakpoint remains unbound | Confirm `emit_maps = true`, then stop and start the Tua launch configuration or command again. |
| The session runs stale generated code | Stop it and start the Tua launch configuration or command again so Tua rebuilds first. |
| The debug build fails | Run **Tua: Focus Output** and fix the reported build diagnostics before relaunching. |
