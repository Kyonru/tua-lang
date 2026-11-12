# Tua for VS Code: Editor reference

Detailed setup, controls, and settings for the Tua extension. Start with the [extension overview](../README.md) for installation and the feature tour. The [language-server guide](https://kyonru.github.io/tua-lang/lsp/) covers editor commands and semantic features.

## Optional Editor Setup

Run **Tua: Set Up Tua** from the Command Palette or the **Tua Tools** menu.
The native Getting Started walkthrough also appears on a fresh installation.
All five steps are optional; opening setup does not change settings.

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
- **Shaders:** check available validators, select a glslang executable, apply or
  restore workspace LOVE shader associations, and optionally install shader
  highlighting from the editor's extension registry.
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
Setup uses no remote content. The optional icon and shader tooling installs contact your editor's
extension registry only when you click their install buttons. The existing separately
opted-in Ollama integration is unaffected.

### Migrating from the personal theme extension

Indigo Carbon now lives in this extension under `themes/indigo-carbon.json`.
Its palette and Tua property-chain rules are preserved. Active tabs match the
editor background; inactive tabs match the distinct tab bar background.
Select **Indigo Carbon HC**, apply **Tua Layout** if desired, then uninstall
`personal-local.indigo-carbon-local` to avoid duplicate theme entries. The old
package's global layout defaults are intentionally not carried into Tua's
configuration defaults. Language and workflow tools work with any theme.

## Tua workspace

Open the **Tua** Activity Bar icon for project files, module exports, signal
call sites, ECS declarations, and the Asset Explorer. Graph groups have an inline
action to open their detailed explorer. Right-click a Tua file for sprite, state-machine, or behavior-tree
previews and generated Lua. Source items navigate to their declaration. Project
data loads on expansion and refreshes after saves; the Refresh action retries
failed requests. Existing commands remain available in the Command Palette.
The sidebar also refreshes after source edits and external file changes. Graph
requests load when you expand their section; use Refresh for an immediate update.

## Named run configurations

Choose Development, Debug, Hot reload, or Package in **Run Configuration**.
The choice is remembered per workspace and drives the compact toolbar Run button,
which opens the game console. Run uses a green play icon and Stop a red square,
with light/dark variants. Direct run commands retain their task terminal.
Customize names and modes in `tua.run.configurations`; packaging configurations
can specify `packageTarget` (`love`, `windows`, or `macos`). Existing launch
configurations and direct commands still work. Run and Debug use the project of
the active file; in a multi-root workspace, open a file from the desired project.
For ad hoc packaging, use the package icon in the Run Configuration header or
**Tua: Package Game...**. Choose the target platform from the picker. If no
project file is active in a multi-root workspace, packaging asks which project
to build. After a successful package task, use **Reveal Package** or **Copy Path**
from the result notification.

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

## Unit tests

Open the Testing view to discover Tua/Busted specs, run a project/file/case, and
view mapped failures. The **Run with Busted** profile supports explicit
continuous runs after saved changes. Discovery is lazy and does not execute
specs. Configure `tua.test.bustedPath` / `tua.test.luaPath` if needed; running
requires Workspace Trust. **Coverage with LuaCov** shows per-file totals and
source-line decorations when LuaCov is installed for the Busted interpreter.
**Tua: Run Unit Tests** keeps the terminal workflow.

## Live runtime inspector

Inspect game state while a local Tua game runs. Inspection is opt-in and adds no
code to release builds.

### Start inspecting

1. Open **Tua → Runtime Inspector** in a trusted workspace.
2. Click **Enable Runtime Inspection**, then start a new Run, Hot reload, or Debug
   session. Restart an existing game to attach.
3. For multiple players, use **Select Runtime Player**.

### Read the view

| Category | What you can inspect |
| --- | --- |
| Stores | State fields and changes since the previous snapshot |
| State machines | Current state and pending transition |
| Entities and components | Sampled entities and their component values |
| Signals | Listener counts and recent event names |
| Object pools | Active, available, and total capacity |

Expand an object to see its values. **Details** holds counters, runtime IDs, and
its creation-site link; empty categories are hidden. Use the inline source action
to jump to Tua when source maps are available.

**Pause/Resume** controls inspector updates, not the game. **Refresh** requests one
snapshot even while paused. After disabling inspection, the toolbar's **Enable**
(plug) action remains available; start a new run after re-enabling it.

Use **Configure Runtime Inspection** (gear) to select collected categories and
an update interval (100–10,000 ms; default 500 ms). Frequency changes apply live;
restart games for category changes. Unselected categories are not wrapped, and
selecting no categories skips the shim on the next launch.

See the [Runtime Inspector guide](https://kyonru.github.io/tua-lang/debugging/#live-runtime-inspector)
for source navigation, troubleshooting, limits, and development-only behavior.

For performance measurements and reproduction steps, see the
[inspector benchmark](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/runtime-inspector/README.md). Pausing updates
or disabling an existing session leaves wrappers installed; restart with
inspection disabled for a run without the shim.

## Contextual hint density

Choose **Contextual** in **Tua: Select Inlay Hint Mode** to hide routine hints.
Select code, run **Inspect Hints at Cursor**, or use **Explain Type** to reveal
complete hints on those lines. While a Lua debug session is active, visible Tua
code in that workspace shows full hints. Moving the selection clears explicit
inspection. This mode is optional and does not change your global hint settings.

Use **Tua: Open Annotation Guide (Offline)** to browse recognized LuaDoc tags
and distinguish checking contracts from metadata-only annotations. Tag
completion and hover include explanations and a link to that guide.

## Contextual declaration tools

Place the cursor inside a direct local `stateMachine(...)`, `objectPool(...)`,
`store(...)`, or `input(...)` declaration and open the lightbulb menu (`Cmd+.` on
macOS, `Ctrl+.` elsewhere). You can also right-click and choose **Tua: Tools for
Declaration…**, or run that command from the Command Palette.

- State machines open their existing static preview.
- Object pools open the explorer with that declaration selected.
- State machines, pools, and stores can select and expand their matching live
  Runtime Inspector object. A picker distinguishes multiple players or instances.
- All four helpers offer their built-in API reference. Input currently has no
  live inspector or dedicated binding preview.

The language server confirms that the constructor is Tua's built-in helper;
shadowed functions and helper aliases do not receive these actions. Runtime
matching requires a connected inspected player and a source-mapped creation site
with the same file, binding name, and line. If inspection is disabled, the action
offers to enable it; start a new run afterward. Restart after moving declarations
to refresh source locations. Static previews retain their existing preview setting.

For completion, hover, diagnostics, code actions, LOVE APIs, and the complete
command list, see the [language-server guide](https://kyonru.github.io/tua-lang/lsp/).

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
- `tua.preview.enabled`: enables experimental sprite-animation, state-machine, and
  behavior-tree previews.
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

## Contributor verification

### Preview tests

Run `npm run check` for unit, syntax, manifest, and activation checks. Run
`npm run test:webviews` for rendered preview interaction tests in an isolated
VS Code profile. To use VSCodium, set `VSCODE_EXECUTABLE_PATH` to its executable.
These tests use a loopback debugging port, a valid PNG fixture, and a controlled
animation clock so desktop occlusion does not affect playback assertions.

### Workspace integration tests

Run `TUA_TEST_SUITE=workspace-ui npm run test:vscode` to exercise the native Tua
sidebar, named run selection, setup checks, contextual hints, session status, and
console source links in an isolated profile with a mock compiler and game runtime.
Set `VSCODE_EXECUTABLE_PATH` to use VSCodium and `TUA_WORKSPACE_SCREENSHOTS` to a
local directory to capture dark, light, and high-contrast views. This suite opens
a loopback debugging port only in its temporary test instance.


Preview layout regression checks can run with an optional Playwright installation:
`PLAYWRIGHT_MODULE=/path/to/playwright-core CHROMIUM_PATH=/path/to/chromium node scripts/webview-scroll-check.js`.
This checks vertical scrolling and bottom-item reachability in wide, short, and
narrow previews, including wrapped toolbars and sticky sprite controls.
