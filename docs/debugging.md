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

Inspect live game state in the Tua sidebar without adding inspector code to your
release build. Use it alongside breakpoints and the static project explorers.

### Enable and connect

1. Open **Tua → Runtime Inspector** in a trusted workspace.
2. Click **Enable Runtime Inspection**.
3. Start a new Tua **Run**, **Hot reload**, or **Debug** session. Existing games
   need a restart.
4. Use **Select Runtime Player** when running multiple players.

Inspection is off by default (`tua.runtimeInspector.enabled = false`). Custom
Lua debug configurations need `tuaProject: true` for Tua launch preparation.

### Read an object

Object rows show a name and a short summary. Expand one to inspect its data;
**Details** contains bookkeeping and source information. Empty categories are
hidden until supported objects appear.

| Category | Main data | Additional detail |
| --- | --- | --- |
| Stores | State fields, with added/changed/removed markers | Updates, last update, subscribers, field watchers |
| State machines | Current state and pending transition | Async phase |
| Entities and components | Entity IDs and component values | Counts represent sampled entities |
| Signals | Listeners and recent events, newest first | Latest 20 event names; no payloads |
| Object pools | Active, available, and capacity counts | Creation site and runtime ID |

Store changes compare consecutive snapshots. Update counters count successful
`set`/`update` calls; direct mutations through `get()` change the displayed state
but do not increment that counter. A new connection starts a fresh comparison.

### Control updates

| Action | Effect |
| --- | --- |
| Pause / Resume Inspector Updates | Freeze or resume the view; the game keeps running |
| Refresh Runtime Snapshot | Request one snapshot, including while paused |
| Disable Runtime Inspection | Disconnect; retain the last snapshot |
| Enable Runtime Inspection (plug icon) | Enable inspection for the next run |

Use **Configure Runtime Inspection** (gear) to choose categories and update
frequency. `tua.runtimeInspector.categories` accepts `ecs`, `states`, `signals`,
`pools`, and `stores` (all enabled by default). Only selected categories receive
runtime wrappers; restart running games to change collection. An empty array
skips the shim entirely on new launches.

`tua.runtimeInspector.updateInterval` sets milliseconds between snapshots:
100–10,000, default 500 (twice per second). Changes apply to connected players
immediately and survive reconnects. Faster updates increase collection cost.
Pause and manual refresh still work at every interval.

Debugger pauses also pause snapshots.
Ended or disconnected players retain their last data with a status and age.

### Navigate between source and runtime

Use the inline **Go to Runtime Object Creation Site** action or
**Details → Creation site**. Source maps lead to Tua; otherwise the link opens
generated Lua. Names such as `player` come from recognizable source bindings.
Ambiguous bindings keep their runtime label; duplicate names include an ID.

To go the other way, place the cursor inside a local `store`, `stateMachine`, or
`objectPool` declaration and choose **Inspect Runtime Instance…** from the
lightbulb menu. Multiple matches open a player/instance picker. These links identify
creation sites, not every subsequent assignment. Restart after moving declarations.

### Troubleshoot

| Symptom | What to check |
| --- | --- |
| No connection after enabling | Restart the game through a Tua run command |
| Cannot find Enable after disabling | Use the plug icon in the inspector toolbar |
| No objects or a missing category | Objects must be alive and use Tua's built-in runtimes |
| Data stopped updating | Check Pause, debugger state, and the selected player's status |
| Source opens generated Lua | Enable `emit_maps`, rebuild, and restart |
| No matching instance from a declaration | Check the selected run, source maps, and whether source lines moved |
| Launcher warning | Check symbolic-link permissions; the game can still run without inspection |

### Limits and development-only behavior

Inspection samples up to 128 objects, 100 entities and 32 component stores per
world, and 3,000 serialized values per snapshot. Field-watcher counts sample up
to 128 watched keys. Truncation is marked; the view is not a complete memory dump.
Custom runtime implementations and arbitrary user tables are not discovered.
The standard LÖVE loop and custom loops returning a frame function are supported.

An extension-owned shim runs through a temporary launcher outside the project.
Each player uses authenticated, non-blocking localhost TCP. It uploads no data,
collects no telemetry, and evaluates no runtime expressions. Source files,
compiler runtimes, normal build output, and packaged games remain unchanged.
Disabling inspection disconnects immediately; the launcher stays until game exit
so asset loading continues. Save identity and player arguments are preserved.

For extension development, `npm run test:inspector` checks two native LÖVE players
and inspector-free packaging. `TUA_INSPECTOR_TEST=1 npm run test:multiplayer`
also checks hot reload and independent debug sessions with inspection enabled.

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
