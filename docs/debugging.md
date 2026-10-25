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

| File             | Used for                                                    |
| ---------------- | ----------------------------------------------------------- |
| `*.lua.map`      | Standard source map v3 consumed by compatible Lua debuggers |
| `*.lua.map.json` | Tua traceback remapping and source/generated navigation     |

Unmapped generated helper lines are skipped while stepping by default. Source
maps currently provide line provenance, so mapped target columns begin at
column zero.

!!! warning "Rebuild after source changes"
Tua builds whenever the command or launch configuration starts. After
editing source, stop the current session and start the configuration again.
Restarting only the existing Lua debug session does not invoke a new Tua
build.

## Live Runtime Inspector

The **Runtime Inspector** in the Tua sidebar shows live ECS entities and
components, active state machines, signal emission/listener counts, and object
pool usage, and store state with update/subscription counts. It complements source breakpoints and the static project explorers.

1. In a trusted workspace, open **Tua → Runtime Inspector** or run
   **Tua: Open Runtime Inspector**.
2. Click **Enable Runtime Inspection**. This sets
   `tua.runtimeInspector.enabled` to `true`; the default is `false`.
3. Launch a new Tua Run, Hot reload, or Debug configuration. Existing processes
   must be restarted. Custom debug configurations need `tuaProject: true`.
4. Use **Select Runtime Player** to choose a player in a multi-instance run.

Snapshots update twice per second. **Pause Inspector Updates** / **Resume Inspector Updates** pauses
snapshot updates without stopping the game; **Refresh Runtime Snapshot** also
works while updates are paused. Debugger pauses suspend snapshots until the game
resumes. Disconnected or ended players retain their last snapshot, labeled with
its status and age. Signal history retains the latest 20 event names, not payloads.
Stores mark state fields added, changed, or removed since the previous snapshot.
Their update counters track successful `set`/`update` calls, not direct mutations
through `get()`; inspection adds no subscribers of its own. Subscriber counts are
exact; field-watcher counts sample at most 128 watched keys and expose that limit.
Large worlds and values are sampled and abbreviated to keep inspection bounded.

The inspector is an **editor-owned development shim**, loaded from a private
temporary launcher outside the project. It connects through authenticated,
non-blocking localhost TCP using LÖVE's bundled LuaSocket. The compiler and
production runtimes contain no inspector hooks; ordinary build output and
packaged games contain no inspector code. It does not upload data or evaluate
runtime expressions. Disabling inspection disconnects the player; the temporary
launcher remains until that player exits so ongoing asset loads still work.

Only Tua's built-in ECS, state-machine, signal, object-pool, and store implementations
are discovered. An empty category can mean that no such objects are currently
alive, or that the game uses a different implementation. Inspection supports the
standard LÖVE loop and custom loops that return a frame function.

## Inspect Generated Lua

Use **Tua: Open Generated Lua Beside Tua** to save and rebuild the active
source, then inspect its freshly emitted file. Use
**Tua: Jump Between Tua and Generated Lua** to move between mapped lines in either direction.

This is useful when a stop lands near lowered class, helper, or compound
assignment code and you need to see the exact Lua executed by LOVE.

## Troubleshooting

| Problem                                                                 | Check                                                                                          |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Second Local Lua Debugger is unavailable or disabled by the environment | Install and enable it in the same VS Code build and profile as Tua.                            |
| LOVE cannot be launched                                                 | Set `tua.love.command` to the LOVE executable and trust the workspace.                         |
| A `.tua` breakpoint remains unbound                                     | Confirm `emit_maps = true`, then stop and start the Tua launch configuration or command again. |
| The session runs stale generated code                                   | Stop it and start the Tua launch configuration or command again so Tua rebuilds first.         |
| The debug build fails                                                   | Run **Tua: Focus Output** and fix the reported build diagnostics before relaunching.           |
