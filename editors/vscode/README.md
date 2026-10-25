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
- **Hints:** choose Full, Compact, Types Only, Parameters Only, or Off for the
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

## Build and run status

The **Run Sessions** view and status bar show builds, running games, reload
requests, successful module reloads, and failed builds that leave the last good
code running. Select a session to inspect project diagnostics and reloaded
modules. Stop beside an active session cancels that run, including hot reload.
Recent sessions stay in memory for this editor window; they are not uploaded.

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

The default `tua.editor.toolbarMode: "compact"` provides four primary controls:
Run/Stop, hot reload, Debug, and **Tua Tools**. The tools menu groups:

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
