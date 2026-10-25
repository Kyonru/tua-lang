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
- Better game-project tooling: sprite sheets, SignalBus events, assets, audio
  systems, module dependencies, ECS, and object pools all have editor support
  instead of living as untyped strings and conventions.
- Generated Lua visibility: inspect emitted Lua beside the current Tua file and
  jump between Tua and generated Lua with source maps.
- Project-oriented workflow: initialize a project, run LOVE from VS Code tasks,
  inspect language-server health, and keep `tua.toml` fully documented in the
  editor.

## Quick Start

1. Install the extension from the
   [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=SolenodonteLabs.tua),
   [Open VSX](https://open-vsx.org/extension/SolenodonteLabs/tua), or a
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

## Optional Editor Setup

Run **Tua: Set Up Tua** from the Command Palette or the **Tua Tools** menu.
The native Getting Started walkthrough also appears on a fresh installation.
All four steps are optional; opening setup does not change settings.

- **Theme:** choose the bundled Indigo Carbon theme, browse other themes, or
  restore the previous theme. Its existing picker name, `Indigo Carbon HC`, is
  retained for compatibility; it is registered as a dark theme, not a formal
  high-contrast accessibility theme.
- **Layout:** move editor actions to the title bar, show the Command Center and
  sidebar/panel controls, and use the compact Tua Tools menu. Run
  **Tua: Restore Previous Layout** to undo the preset.
- **File icons:** optionally install/select Material Icon Theme from your
  editor's configured extension registry. It is not a dependency. Restore changes
  the selected file icons; it leaves that extension installed. Product icons are
  unchanged.
- **Hints:** choose Contextual, Full, Compact, Types Only, Parameters Only, or Off for the
  current workspace (or user profile when no folder is open), even with no Tua
  file open.

Theme, layout, and icon presets save the previous explicit user settings locally
before writing changes. Repeated Apply clicks preserve the original backup.
Restore removes settings that were previously unset and preserves settings you
edited manually after applying. Workspace and folder overrides are never changed;
setup reports when they prevent a preset from taking effect. Hint modes use the
existing Tua hint controls and their workspace scope.

The layout preset changes only `window.commandCenter`,
`workbench.editor.editorActionsLocation`, `workbench.layoutControl.enabled`,
`workbench.layoutControl.type`, and `tua.editor.toolbarMode`.
Installing Tua or selecting its theme never applies the layout preset.

The theme and setup add no telemetry, account requirements, or trial period.
Setup uses no remote content. The optional icon install contacts your editor's
extension registry only when you click its button. The existing separately
opted-in Ollama integration is unaffected.

### Migrating from the personal theme extension

Indigo Carbon now lives in this extension under `themes/indigo-carbon.json`.
Its palette, lighter active tabs, and Tua property-chain rules are unchanged.
Select **Indigo Carbon HC**, apply **Tua Layout** if desired, then uninstall
`personal-local.indigo-carbon-local` to avoid duplicate theme entries. The old
package's global layout defaults are intentionally not carried into Tua's
configuration defaults. Language and workflow tools work with any theme.

## Tua workspace

Open the **Tua** Activity Bar icon for project files, module exports, signal
call sites, ECS declarations, and the Asset Explorer. Graph groups have an inline
action to open their detailed explorer. Right-click a Tua file for sprite/state
previews or generated Lua. Source items navigate to their declaration. Project
data loads on expansion and refreshes after saves; the Refresh action retries
failed requests. Existing commands remain available in the Command Palette.

## Named run configurations

Choose Development, Debug, Hot reload, or Package in **Run Configuration**.
The choice is remembered per workspace and drives the compact toolbar Run button,
which opens the game console. Run uses a green play icon and Stop a red square,
with light/dark variants. Direct run commands retain their task terminal.
Customize names and modes in `tua.run.configurations`; packaging configurations
can specify `packageTarget` (`love`, `windows`, or `macos`). Existing launch
configurations and direct commands still work. Each action uses the project of
the active file; in a multi-root workspace, open a file from the desired project.

### Multiple local players

Run, Hot reload, and Debug configurations can launch up to 16 named instances from one build:

```json
"tua.run.configurations": [
  {
    "name": "Local multiplayer",
    "mode": "run",
    "instances": [
      { "name": "Host", "args": ["--host", "22122"] },
      { "name": "Client", "args": ["--join", "localhost:22122"] }
    ]
  }
]
```

Select **Local multiplayer** and press Run. Each player gets its own process and
console session. Run and Hot reload pass arguments literally without shell
expansion; Debug uses your installed debugger's argument handling. Your game
must interpret them (for example in `love.load(args)`) and implement the
host/client networking. Omit `args` to launch identical copies.

The primary Stop button stops the selected configuration’s group. Stop beside an individual player
in **Run Sessions** stops only that player. Closing or crashing one player leaves
its peers running. **Tua: Stop LOVE Project** stops all tracked normal runs;
**Tua: Restart LOVE Project** stops those runs and launches the last normal Run
configuration again, preserving player names and arguments. A failed build starts
no players. Instances share the build output and LÖVE save identity; games that
write player-specific saves should choose distinct identities themselves.

Use `"mode": "hotReload"` with the same `instances` to share one build watcher.
Every player reloads after a successful rebuild; build errors leave the previous
code running. Closing one player leaves the watcher and other players active.
Stopping the group or closing its last player also stops the watcher.
This mode requires the bundled CLI's `tua dev --no-launch` support; update a custom
`tua.server.path` executable along with the extension.

Use `"mode": "debug"` to launch separate Lua debugger sessions. Set breakpoints in
`.tua` files, then select the player in VS Code's debug session picker to step or
continue it independently. The group uses an isolated copy of one build, with
source maps pointing to your original sources. Its files are cleaned up after the
last player exits. A partial launch failure stops the players already launched.
Debugging requires the supported Local Lua Debugger extension, as for a single
player. `instances` is not supported for packaging.

The primary Play button becomes Stop while the selected configuration is active,
including during its build. For a group, Stop ends all of its players. Selecting
another idle configuration shows Play for that configuration; individual player
Stop actions remain available in **Run Sessions**.

## Build and run status

The **Run Sessions** view and status bar show builds, running games, reload
requests, successful module reloads, and failed builds that leave the last good
code running. Select a session to inspect project diagnostics and reloaded
modules. Stop beside an active session cancels that run, including hot reload.
The primary Stop action targets the selected configuration; each session also
has its own Stop action. Recent sessions stay in memory for this editor window; they are not uploaded.

## Game console

Open **Tua: Open Game Console** for run, hot-reload, build, and debugger output.
Filter by severity, module, or session; consecutive identical messages collapse
into a count. Source-mapped errors open their Tua location. Structured tags such
as `[warn] [combat] low health` identify severity and module; untagged output uses
conservative text/stderr classification. The original task terminal stays available.
The console keeps the latest 2,000 entries in memory and caps each message at
16,000 characters. Clear removes captured messages; nothing is transmitted.

## Project checks

Run **Tua: Check Project Setup** or use the action in **Project Checks** to verify
the compiler, LOVE executable, debugger extension, and source maps. Each finding
has a relevant action: select a compiler or LOVE executable, install the optional
debugger, enable source maps, or build the project. Source-map edits stay open
for review and saving. Executable probes run only after you request checks in a
trusted workspace, with a three-second timeout. **Tua: Build Project** builds
without launching the game.

## Live runtime inspector

Open **Tua → Runtime Inspector**, click **Enable Runtime Inspection**, then launch
Run, Hot reload, or Debug. Use **Select Runtime Player** to choose a player in a
multi-instance run. Inspection is off by default and requires a trusted workspace.
Enabling it applies to new runs; restart an already-running game to attach.
The setting is `tua.runtimeInspector.enabled` (default `false`). For a custom
debug launch configuration, use `tuaProject: true` to enable Tua launch preparation.

The view shows live objects from Tua's built-in runtimes:

- **Entities and components:** entity IDs and component values in each ECS world.
- **State machines:** current state and pending asynchronous transition.
- **Signal activity:** emission counts, the latest 20 event names, and listener
  counts. Signal payloads are not retained.
- **Object pools:** active, available, and capacity counts.
- **Stores:** current state, successful `set`/`update` call counts, seconds since
  the last successful call, and subscriber/field-watcher counts. State fields
  added, changed, or removed since the previous snapshot have explicit markers.
  Direct mutations through `get()` appear in state snapshots but do not increment
  the API update count; calls that raise errors are not counted. First snapshots
  and connections after a restart start a new comparison baseline. Watcher counts
  sample at most 128 watched keys and report when that limit is reached.

Snapshots arrive twice per second. **Pause Inspector Updates** / **Resume Inspector Updates** pauses
snapshot updates without pausing the game; **Refresh Runtime Snapshot** requests a
snapshot even while updates are paused. A stopped or disconnected player retains
its last snapshot with an explicit status and age. A debugger pause also stops
new snapshots until the game resumes.

The inspector is an extension-owned development shim, loaded by a private
**temporary launcher outside the project**. It wraps runtime constructors only
inside that game process. It does not modify source files, compiler runtimes,
normal build output, or packaged games. Run and hot reload mirror current build
output; multiplayer debugging retains its isolated build snapshot. Temporary launchers are
removed when their player exits. Disabling inspection disconnects immediately
and keeps the launcher alive until the game exits so asset loading continues.

Each player connects to its own authenticated listener on `127.0.0.1` using
LÖVE's bundled LuaSocket. There is no remote listener, upload, telemetry, or
runtime expression evaluation. Networking is non-blocking and bounded; a slow
editor skips snapshots instead of growing a queue. Large worlds and values are
sampled (128 live objects, 100 entities per world, 32 component stores per world,
3,000 serialized values per snapshot), and abbreviated values are marked.

This currently inspects Tua's built-in ECS, state-machine, signal, pool, and store
implementations. Other ECS backends and arbitrary user tables are not discovered.
It supports the standard LÖVE loop and custom loops that return a frame function.
Platforms that restrict creating symbolic links need that permission enabled;
if a launcher cannot be created, the game runs normally and a warning explains
why inspection is unavailable. Save identity and player arguments are preserved.

Validation: `npm run test:inspector` exercises two native LÖVE players and checks
that packaging after inspection contains no shim. Run `TUA_INSPECTOR_TEST=1`
with the existing `test:multiplayer` harness to check hot reload, source-mapped
breakpoints, and independent debug sessions with inspection enabled.

## Contextual hint density

Choose **Contextual** in **Tua: Select Inlay Hint Mode** to hide routine hints.
Select code, run **Inspect Hints at Cursor**, or use **Explain Type** to reveal
complete hints on those lines. While a Lua debug session is active, visible Tua
code in that workspace shows full hints. Moving the selection clears explicit
inspection. This mode is optional and does not change your global hint settings.

## What You Get

### Core Tua Editing

- TextMate and semantic highlighting for Tua, Lua interop, LOVE-aware helpers,
  event names, annotations, types, functions, methods, parameters, properties,
  labels, and literals.
- Zero-delay quick suggestions for Tua files using VS Code's normal completion
  flow.
- Opt-in deterministic ghost text from unique statement snippets and
  conservative repeated-line patterns in the current file. It stays local,
  runs before the separately opted-in Ollama fallback, and does not build a
  project corpus or use cross-project history in this first phase. A local
  request that has not answered within 250 ms after the 65 ms debounce is
  cancelled and does not start Ollama.
- Hover, completion, signature help, definition, references, rename, document
  symbols, workspace symbols, document highlight, call hierarchy, code lenses,
  and range/full/delta semantic tokens.
- Inlay hint modes: Contextual, Full, Compact, Types Only, Parameters Only, and Off.
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
- `Tua: Run LOVE Project`, `Tua: Stop LOVE Project`, and `Tua: Restart LOVE
  Project` as cancellable VS Code tasks. Runtime `print(...)`, stdout, and
  stderr stream into the focused task terminal, and emitted Lua traceback lines
  are mapped back to original Tua files when source maps are enabled.
- `Tua: Run LOVE Project With Hot Reload` runs `tua dev`, so saved changes
  reload into the running game, and `Tua: Package Game...` builds a `.love`,
  fused Windows, or fused macOS package with `tua package`.
- `Tua: Debug LOVE Project` bridges generated Lua to the optional
  [Second Local Lua Debugger](https://marketplace.visualstudio.com/items?itemName=ismoh-games.second-local-lua-debugger-vscode)
  extension. Breakpoints stay in `.tua`; stepping, variables, watches,
  conditional breakpoints, and evaluation remain standard Lua debugging. Game
  workspaces can also opt a `lua-local` launch configuration into the same
  preparation pipeline with `tuaProject: true`.
- Compiler-backed sprite-sheet and state-machine previews. State graphs expose
  initial states, event routes, wildcard transitions, cycles, and source
  navigation without executing project code, with reserved routing corridors
  that keep return paths and self-loops clear of state cards. Hover links open
  sprite, state-machine, and recognized asset previews directly. Experimental
  previews are gated by `tua.preview.enabled`.

### Game-Focused Project Tooling

- Asset Explorer grouped by asset kind and static usage, with reference
  navigation, rename, reveal, and preview. Stable tree identities preserve expansion
  as counts change; **No static references** can include dynamically loaded assets.
- SignalBus event-name completion and a compiler-backed signal graph with
  typed payload display, conflict inspection, and source navigation.
- Module dependency explorer for `require(...)` and `import(...)` relationships,
  unresolved modules, duplicate resolutions, cycles, reverse dependencies, and
  inclusion paths.
- ECS explorer showing components, systems, queries, and conservative read/write
  access insights.
- Object pool explorer showing declarations plus visible `acquire`, `release`,
  `clear`, and `prewarm` lifecycle operations.
- Audio Explorer showing static buses, cues, effects, music scenes, layers,
  analysis metadata, and source navigation without running the project.
- State-machine graph preview for static `stateMachine({...})` definitions,
  with live edit refresh and source-linked states and event routes.

## Key Commands

### Project and Runtime

- `Tua: Initialize Tua Project`
- `Tua: Run LOVE Project`
- `Tua: Stop LOVE Project`
- `Tua: Debug LOVE Project`
- `Tua: Restart LOVE Project`
- `Tua: Run LOVE Project With Hot Reload`
- `Tua: Package Game...`
- `Tua: Open Runtime Inspector`
- `Tua: Enable Runtime Inspection`
- `Tua: Disable Runtime Inspection`
- `Tua: Select Runtime Player`
- `Tua: Pause Inspector Updates`
- `Tua: Resume Inspector Updates`
- `Tua: Refresh Runtime Snapshot`
- `Tua: Focus Output`
- `Tua: Open Generated Lua Beside Tua`
- `Tua: Jump Between Tua and Generated Lua`

### Editing and Insight

- `Tua: Explain Type / Why Is This Any?`
- `Tua: Select Inlay Hint Mode`
- `Tua: Toggle Experimental Completions`
- `Tua: Toggle Deterministic Inline Completions`
- `Tua: Toggle Local Ollama Completions`
- `Tua: Test and Warm Ollama Model`
- `Tua: Copy Fully Qualified Symbol Name`
- `Tua: Workspace Symbols by Module`
- `Tua: Organize Imports`
- `Tua: Sort Imports`
- `Tua: Remove Unused Imports`
- `Tua: Fix All Safe Diagnostics`

### Project Explorers

- `Tua: Refresh Asset Explorer`
- `Tua: Preview Sprite Animations`
- `Tua: Preview State Machine`
- `Tua: Show Signal Graph`
- `Tua: Show Module Dependency Graph`
- `Tua: Show ECS Explorer`
- `Tua: Show Object Pool Explorer`
- `Tua: Show Audio Explorer`
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
- `tua.runtimeInspector.enabled`: opt into read-only live inspection for new
  development runs; default `false`. No inspector code is added to release output.
- `tua.trace.server`: LSP trace level (`off`, `messages`, `verbose`).
- `tua.log.verbosity`: server log verbosity (`off`, `error`, `info`, `debug`,
  `trace`).
- `tua.preview.enabled`: enables experimental sprite-animation and
  state-machine previews.
- `tua.completion.sequenceRanking.enabled` and
  `tua.completion.multiToken.enabled`: experimental completion lanes controlled
  together by `Tua: Toggle Experimental Completions`.
- `tua.completion.inline.enabled`: opts into experimental deterministic inline
  completion. `tua.completion.inline.sources.semantic` and
  `tua.completion.inline.sources.currentFile` independently control its two
  Phase 1 sources. Use `Tua: Toggle Deterministic Inline Completions` to change
  only this gate for the current workspace.
- `tua.completion.multiToken.allowModelIntegration`: separately opts into the
  loopback Ollama fallback. Deterministic completion runs first when both are
  enabled.

The Tua inline-completion language status reports `Local`, `Ollama`,
`Local + Ollama`, or `off`, so enabling one backend never makes the other look
enabled. Copied debug information includes only source-free counters. For the
deterministic backend, `semanticReturned`/`semanticAccepted` and
`currentFileReturned`/`currentFileAccepted` mean items returned to VS Code and
items accepted by the user; they do not affect ranking.

The extension also changes a few VS Code editor defaults for Tua files only,
whether or not inline completion is enabled: suggestions appear immediately in
code and strings (`editor.quickSuggestions`, `editor.quickSuggestionsDelay`),
semantic highlighting is on, and `editor.suggest.preview` is `true`, so the
selected menu item is previewed in the editor. Override any of them in a
`"[tua]"` block in your settings.

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
- [Audio systems](https://kyonru.github.io/tua-lang/audio/)
- [Examples](https://kyonru.github.io/tua-lang/examples/)

## Privacy and Support

The extension does not send telemetry or upload project source code.

`Tua: Copy Debug Info` gathers extension version, VS Code version, platform,
workspace trust, resolved server path, server version, server state, workspace
directory, and recent startup failure text when available. It does not copy
source file contents.

`Tua: Report Issue` opens the project issue URL with the same diagnostic bundle
pre-filled for review before submission.

## Property-chain colors

Property accesses carry optional semantic modifiers `propertyChain1` through
`propertyChain4`, cycling from the first member of each chain. For example,
`game.player.weapon.stats.damage.base` reads a weapon’s base damage. Its properties
cycle through blue, teal, peach, lavender, then blue again in Indigo Carbon.
The root retains its ordinary semantic color. Calls and indexes preserve the
receiver's position; chains within their arguments start independently. Methods
keep their ordinary method color but count as a step in the receiver chain.

Themes can style `property.propertyChain1:tua` through
`property.propertyChain4:tua`. Themes without these rules retain normal property
highlighting. To opt in through user settings with any theme:

```json
{
  "editor.semanticTokenColorCustomizations": {
    "rules": {
      "property.propertyChain1:tua": "#8EC7FF",
      "property.propertyChain2:tua": "#88D5CF",
      "property.propertyChain3:tua": "#F0BFA5",
      "property.propertyChain4:tua": "#C5ACF5"
    }
  }
}
```

Semantic highlighting must be enabled. Property declarations do not receive
chain modifiers; existing readonly, modification, and deprecation markers remain.

## Editor toolbar density

The default `tua.editor.toolbarMode: "compact"` shows **Run Selected Configuration**,
the configuration picker, Stop while a session is active, and **Tua Tools**.
The tools menu groups:

- Code: inlay hint mode, generated Lua, and jumping between source and output.
- Structure: workspace symbols, signal graph, and state-machine preview.
- Run: restart the LOVE project.

The menu follows the active editor: Lua files expose the source/output jump;
Tua files expose the full set. All commands remain available in the Command
Palette. Set `tua.editor.toolbarMode` to `full` to restore individual buttons.

With `workbench.editor.editorActionsLocation: "titleBar"`, these controls appear
to the right of the Command Center. The extension groups the tools independently
of any color theme, leaving space for VSCodium's sidebar and panel toggles.

## Sprite animation preview

Sprite previews retain the selected animation, paused frame, speed, and zoom when
reloaded for the same source file. Switching animations keeps the current pause
choice; Play restarts a completed non-looping animation. Playback stops requesting
animation frames while paused or hidden, and honors reduced motion on first open.
The preview supports up to 2,048 frames per animation and reports invalid ranges,
out-of-image frames, and image decode failures. The error view includes Reload.

Wide sprite panels place the image and controls side by side. Narrow panels stack
them without a sticky image covering the controls. Frame buttons remain stable
while playing so keyboard focus is retained.

## State-machine preview

State-machine previews retain the selected machine, state or transition, and graph
scroll position across refreshes. Selecting a node or event keeps its button and
keyboard focus intact; double-click opens its source. These are static source
previews; they do not execute the game or display its current runtime state.

## Preview verification

Run `npm run check` for unit, syntax, manifest, and activation checks. Run
`npm run test:webviews` for rendered preview interaction tests in an isolated
VS Code profile. To use VSCodium, set `VSCODE_EXECUTABLE_PATH` to its executable.
These tests use a loopback debugging port, a valid PNG fixture, and a controlled
animation clock so desktop occlusion does not affect playback assertions.

## Workspace integration verification

Run `TUA_TEST_SUITE=workspace-ui npm run test:vscode` to exercise the native Tua
sidebar, named run selection, setup checks, contextual hints, session status, and
console source links in an isolated profile with a mock compiler and game runtime.
Set `VSCODE_EXECUTABLE_PATH` to use VSCodium and `TUA_WORKSPACE_SCREENSHOTS` to a
local directory to capture dark, light, and high-contrast views. This suite opens
a loopback debugging port only in its temporary test instance.
