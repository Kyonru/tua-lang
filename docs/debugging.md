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
   You can do this from **Tua: Set Up Tua → Add game-development tools**.
3. Open the folder containing `tua.toml` and place a breakpoint in a `.tua`
   file.
4. Open **Run and Debug → Select and Start Debugging**, then choose **Debug Tua
   LOVE Game**. You can also run **Tua: Debug LOVE Project** from the Command
   Palette.

Both workflows build the project before launching LOVE. If LOVE is not on
`PATH`, set `tua.love.command` to the executable path in VS Code settings.

Keep source-map emission enabled in `tua.toml`:

```toml
emit_maps = true
```

This is the default for new projects.

!!! important "Opt the launch configuration into Tua"
    A normal `lua-local` configuration bypasses Tua's build and source-map
    preparation. The suggested **Debug Tua LOVE Game** configuration includes
    `tuaProject: true`. Add it to any custom configuration as shown below so
    Tua can prepare and restore the generated output correctly.

## Run From The VS Code UI

The **Select and Start Debugging** picker offers **Debug Tua LOVE Game** when
the workspace folder contains `tua.toml`. Tua builds the project and launches
LOVE through Second Local Lua Debugger. No `launch.json` is needed for this
default configuration.

To customize debugger options or keep a named configuration in the Run and
Debug dropdown, create or update `.vscode/launch.json`:

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

When you start the **Debug** run configuration or **Tua: Debug LOVE Project**,
Tua opens VS Code's **Debug Console**. The Lua debugger forwards LOVE's
standard output and error output there, including `print(...)`. The Tua **Game
Console** also receives those debugger output events. The **Development** run
configuration is a normal task, so its game output appears in the run terminal
and Tua Game Console; it has no Debug Console session. Choose **Debug** when you
want logs beside breakpoints and variable inspection.

Neither workflow adds debugger code to project source.

## Find game logs and runtime errors

Run output appears in the bottom-panel **Tua Console → Game Console**. After a
run starts, the status bar also shows **Game Logs**; click it to reopen the
console. The Run configuration opens it automatically. A failed normal run
opens it as the game exits. Debug sessions also write to VS Code's **Debug
Console**.
When a game is running, the console header also has **Stop Session**.

The Game Console keeps the latest 2,000 messages, collapses consecutive
duplicates, and can filter by level, scope, or run session. A mapped message
shows `file.tua:line` beside it. Click the row to open that line. For generated
Lua traceback lines, keep `emit_maps = true` so Tua can map them to `.tua`.
When a generated line has no exact map, Tua leaves it unlinked rather than
guessing a source line; inspect the full traceback in the run terminal or
Debug Console.

For structured game messages, Tua provides an optional `Log` global:

```lua
local combat = Log.scope("combat")
combat.info("enemy spawned", enemyId)
combat.warn("low health", health)
Log.error("asset failed", assetPath) -- logs; does not throw
```

Levels are `trace`, `debug`, `info`, `warn`, and `error`. The default minimum is
`info`; call `Log.setLevel("debug")` during development to see debug messages.
Arguments are converted to text and separated by tabs. Newlines are escaped
so each call stays one console entry. When Lua's debug library is available,
each message includes its generated Lua call site; the Game Console maps that
location to Tua when source maps exist.

The logger runs only when called, and its runtime is copied into a build only
when Tua source uses `Log`. In a plain `.lua` file, write
`local Log = require("tualog")` explicitly. For a real exception and traceback,
use Lua's `error(...)` instead of `Log.error(...)`.

## Source-Map Files

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
For source-mapped state machines, signals, ECS worlds, and object pools, open
the corresponding static diagram or explorer from **Details**. The inspector
continues to show live values; these other views show declared relationships,
not runtime progress.

| Category | Main data | Additional detail |
| --- | --- | --- |
| Stores | State fields, with added/changed/removed markers | Updates, last update, subscribers, field watchers |
| State machines | Current state and pending transition | Async phase |
| Entities and components | Entity IDs and component values | Counts represent sampled entities |
| Signals | Listeners and recent events, newest first | Latest 20 event names; no payloads |
| Object pools | Active, available, and capacity counts | Peak simultaneous active count and rejected releases since creation; creation site and runtime ID |

Store changes compare consecutive snapshots. Update counters count successful
`set`/`update` calls; direct mutations through `get()` change the displayed state
but do not increment that counter. A new connection starts a fresh comparison.
To inspect the sequence of store updates, enable **Store change history** in
**Configure Runtime Inspection** or set `tua.runtimeInspector.storeHistory` to
`true`, then start a new run. The inspector keeps the latest 20 successful
`set`/`update` calls per store, with a bounded copy of the resulting state.
History is off by default and does not retain references to game tables.
Pool pressure counts are lifetime values for each inspected pool. A rejected
release is a `release` call that returned `false`, including a foreign or
already released object; a call that raises an error is not counted.

### Control updates

| Action | Effect |
| --- | --- |
| Pause / Resume Inspector Updates | Freeze or resume the view; the game keeps running |
| Refresh Runtime Snapshot | Request one snapshot, including while paused |
| Disable Runtime Inspection | Disconnect; retain the last snapshot |
| Enable Runtime Inspection (plug icon) | Enable inspection for the next run |

Use **Configure Runtime Inspection** (gear) to choose categories and update
frequency. `tua.runtimeInspector.categories` accepts `ecs`, `states`, `signals`,
`pools`, `stores`, `inputs`, `behaviors`, and `audio` (all enabled by default). Only selected categories receive
runtime wrappers; restart running games to change collection. An empty array
skips the shim entirely on new launches.

The Input category samples each Baton's active device, configured control
sources, post-deadzone values, directional pairs, and pointer position when
enabled. Values are sampled at the inspector interval, so one-frame press and
release edges may fall between snapshots. No input data is collected unless
runtime inspection is enabled for a development run.

The Behavior runners category samples active node paths after each runner tick
and keeps the latest 20 root results with step counts. A completed tree has no
active nodes; the history still shows its result. This is sampled runtime state,
not a trace of every node visited during a tick. The shim wraps runners only
in the temporary editor launcher and never changes packaged games.

The Audio systems category shows effective bus gains, voice counts, music
state, started/stolen/rejected totals, and up to 64 active cue records. The
source-linked **Open static Audio Workbench** action shows authored cues and
scenes separately. The inspector retains no LÖVE Source objects.

`tua.runtimeInspector.updateInterval` sets milliseconds between snapshots:
100–10,000, default 500 (twice per second). Changes apply to connected players
immediately and survive reconnects. Faster updates increase collection cost.
Pause and manual refresh still work at every interval.

Unexpected connection loss retries automatically; explicit Disable stops the
connection. Disabling an existing game leaves wrappers until that game exits.
Debugger pauses also pause snapshots.
Ended or disconnected players retain their last data with a status and age.

## Capture LÖVE Performance

Open the **Run and Debug** sidebar and expand **Tua LOVE Game**, then select
the **Profile LOVE Game** pulse button. The same action is available in
**Tua → Run Configuration** and as **Tua: Profile LOVE Game** in the Command
Palette. The button changes to **Stop LOVE Profile Capture** while a capture
is being prepared or recorded. Stopping a capture leaves the game running.
The Tua view shares its configurations and selection with the Tua sidebar;
the ordinary VS Code debugger toolbar continues to control breakpoint
sessions.

Run the command in a trusted project workspace. Choose a 10- or
30-second capture, or **Until stopped**. Optionally select ECS systems,
behavior trees, signals, audio systems, or asset/shader creation for named
tracing, then enter a frame budget in milliseconds. Tua builds the project
and launches LÖVE from a private,
temporary development directory. **Tua: Stop LOVE Profile** ends a manual
capture after the current frame; the game keeps running.

The resulting view shows the median, 95th percentile, and worst **retained**
frame, plus frames over the chosen budget. It keeps the last 2,048 frames and
at most 512 distinct CPU stacks. Frame wall time includes the game loop and
sleep; update and draw columns time those callbacks separately. Draw-call
counts come from `love.graphics.getStats`, and the GC column counts LuaJIT
samples in GC state. These measurements help correlate a spike; they do not
prove that a particular callback caused it.

CPU stacks come from LuaJIT's statistical profiler. Click a mapped stack to
open its `.tua` line, or **Lua** to inspect the generated line. Enable
`emit_maps = true` for source links. Sampling may miss short work, and JIT
inlining can make line attribution approximate. A capture with no project
samples still shows frame data. If this LÖVE build lacks `jit.profile`, the
view states that CPU sampling was unavailable.

When timed CPU samples are available, **Open flame chart** opens a converted
`.cpuprofile` in the optional [Flame Chart Visualizer for JavaScript Profiles](https://marketplace.visualstudio.com/items?itemName=ms-vscode.vscode-js-profile-flame).
If it is missing, Tua offers to install it. **Export .cpuprofile** in the capture
view, or **Tua: Export Last LOVE CPU Profile** in the Command Palette, saves a
copy at a location you choose. The command uses the most recently completed
capture and explains when it has no timed CPU samples. Open the saved file later
in VS Code or VSCodium to inspect it. The export has one timeline point per
LuaJIT profiler callback, retaining the latest 4,096 callbacks. A callback may
represent several samples without separate timestamps, so the flame timeline
and source lines are approximate.
For longer captures, the chart starts at the earliest retained callback and
the results view names the number of callbacks evicted from its timeline.
Older captures and captures without timed samples have no export action. This
conversion is a CPU sample profile, not a V8 or Lua heap profile.

Named traces time selected runtime boundaries and show call count, total time,
and slowest call. Their totals overlap when one boundary invokes another; do
not add them together as whole-frame time. Trace wrappers are installed only
for selected categories and restored when capture ends. The latest 32
completed traced calls are retained for failure context, without event
payloads. A failure capture also links the LÖVE traceback to Tua source when
the generated line has a source-map entry. Recent calls are context, not proof
that any one call caused the failure.
Errors while loading the game's main module also produce a failure report;
CPU sampling has not begun at that point, so those reports have no CPU samples.

The profiler shim is loaded only by the temporary launcher. Ordinary runs,
builds, and packaged games contain no profiler code or per-frame profiler
branch. Captures have measurement overhead; compare runs with and without
capture before using a small timing difference as evidence.

For a reproducible fixture-based overhead comparison on a machine with LÖVE,
run `npm --prefix editors/vscode run bench:profiler` from the Tua repository.
It runs three ordinary/capture pairs and reports median and 95th-percentile
CPU-loop and frame intervals. Those host-specific figures are a probe of the
profiler, not a correction factor for your game.

### Record and replay input

Select **Record keyboard and mouse input for replay** when starting a capture.
The report keeps at most 4,096 key, text, mouse, and wheel events with their
frame numbers; it excludes file drops, joystick objects, and arbitrary event
payloads. A generated seed initializes Lua and LÖVE random generators before
the game loads. The report shows the event count and whether the limit was
reached.

Use **Replay with current build** in the result or **Tua: Replay Last LOVE Input
Capture**. Tua rebuilds the same project and launches it from another temporary
directory. Live keyboard and mouse events are ignored during replay, except a
window quit. Replay uses the recorded frame order and seed, but it does not
reproduce frame durations, wall-clock time, external I/O, network events,
arbitrary Lua state, or seeds set later by game code. If any supported input
event was dropped, replay is unavailable. Treat it as a debugging aid, not a
determinism guarantee. The recording is held only in the current editor
session and is not added to the game build.

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

The State Machine preview can show the last sampled current state when opened
from a selected inspected instance. Its routes still come from source analysis;
the runtime state is labeled with its session and sample age. This overlay is
available only in the development editor session.

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
