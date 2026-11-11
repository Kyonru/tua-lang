# Code-first audio workbench design

Status: design proposal, 2026-11-12. No new audio API or editor behavior is implemented by this document.

## What a game developer should be able to do

1. Write a cue in Tua, trigger it with a game event, hear it in isolation, and see which asset or generated sound it uses.
2. Adjust gain, pitch, fade, voice limit, bus, and supported effect settings while hearing the result. The saved Tua source remains authoritative.
3. Arrange music as named scenes and synchronized layers, inspect tempo and loop boundaries, then test scene transitions at a beat or bar.
4. Make short sound effects with code when an asset is inconvenient, without allocating sample buffers every frame.
5. Find a silent cue, clipped or crowded voice group, unsupported effect, or mismatched loop before running the whole game.

The first three build on `audioSystem({...})` as documented in `docs/audio.md`. The fourth needs a small library/API addition; Tua currently demonstrates procedural SoundData generation in `examples/audio-system/src/main.tua`, but has no dedicated sound-design API. A step sequencer or algorithmic music scheduler would be a separate capability.

## Current boundary

`tua/audioGraph` currently returns the declaration location and **names only** for buses, cues, effects, scenes, and layers (`crates/tua-core/src/audio.rs`, `crates/tua-lsp/src/server/protocol.rs`). The Audio Explorer renders those names as pills and separately reads generated `audio-index.json` tempo analysis (`editors/vscode/src/audio/index.js`). It cannot accurately display a cue's source, routing, parameters, scene membership, timing, or playable waveform from that response. The existing Audio Explorer is an inventory, not an authoring or audition tool.

LÖVE's `Source` supports playback, pitch, volume, positioning, filters, and effects. Effects must be defined with `love.audio.setEffect`, and platform support must be checked. A `SoundData` can generate samples for a static Source. Streaming sources suit long music files; repeatedly constructing a Source in `love.update` is costly. LÖVE does not provide native DSP mixer buses; Tua's buses are control groups that apply settings to Sources. These constraints must remain visible in the UI.

## Inspiration and choice

Strudel's useful ideas are code as the editable artifact, quick audition, and feedback tied to the events that sound. Its pattern mini-notation, browser synthesis engine, and continuously running REPL are not drop-in LÖVE features.

| Direction | What it would help with | Cost or limitation |
| --- | --- | --- |
| **Mixer canvas**: connected bus/effect nodes, meters, sliders | Routing and bus gain | Implies native mixer buses and live levels that Tua does not have; weak for authoring one cue or music transition. |
| **Cue and scene workbench**: code editor stays primary; side panel selects a cue or scene, auditions it in a LÖVE process, and shows its authored source, voice policy, routing, timing, and diagnostics | The edit → hear → locate loop; game-event audio and adaptive scenes | Requires richer compiler data and a controlled audition process. |

Choose the **cue and scene workbench**. Keep the existing Audio Explorer command, but evolve it into a task view with two domain modes: **Cues** and **Music scenes**. Do not add a generic dashboard of all audio metrics.

## Proposed interface

```text
Audio Workbench                      [Refresh] [Stop audition]
System: src/audio.tua              Static configuration · revision 42

┌ Cues ──────────────┬ Selected: enemy.hit ───────────────────────┐
│ Search             │ [Play] [Open source] [Open asset]          │
│ enemy.hit          │ Source: assets/hit.wav · 0.18 s · mono     │
│ player.jump        │ [waveform / playhead when decoded]         │
│ ui.confirm         │ Bus: sfx → master · gain: authored 0.8     │
│                    │ Voices: max 4 · overflow: steal            │
│                    │ Effect: cave-reverb · support: available   │
│                    │ Issue: peak may clip / source missing     │
└────────────────────┴─────────────────────────────────────────────┘

Music scenes | exploration | combat
Selected: combat · BPM 120 (authored) · 4/4 · transition: bar
bar 1       bar 2       bar 3       bar 4
drums  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  [Open layer]
bass   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  [Open layer]
```

The mockup shows intended information architecture, not existing data. Only show a waveform when an asset was decoded; only show a playhead when a real audition is running. Show analysis BPM with confidence and distinguish it from authored BPM. Show an effect as source-applied or inherited control-bus effect, never as a native bus processor. A static declaration must not display fabricated voice meters or active-game state. A separate runtime mode can use `snapshot()` and events from a connected game, labeled **Live**.

The list should remain usable at narrow width: selected detail moves below the list, no horizontal page overflow, and the primary Play/Open actions stay visible. Keyboard navigation follows VS Code conventions; Play and Stop have accessible names; moving focus into the webview and back to source is predictable. Respect theme tokens, high contrast, and reduced motion. A waveform uses a text duration/position fallback for screen readers.

## Data and execution contract

1. Extend the existing core audio query with stable item IDs, source spans, source expression kind (`asset`, `SoundData`/computed, or unknown), cue-to-bus/effect references, authored options, scene/layer relationships, and timing provenance. Emit `unknown` for dynamic values rather than guessing. All static facts come from the same compiler snapshot as diagnostics and navigation.
2. Join asset metadata by project and canonical asset path, with explicit `missing`, `unsupported`, `stale`, or `not analyzed` states. Waveform peaks are optional cached derived data, bounded in size. Tempo analysis remains a suggestion with confidence, not the score's truth.
3. Audition in an isolated LÖVE child process using generated Lua or a small harness that loads the project's audio declaration. Never execute arbitrary project code inside the VS Code webview or extension host. Require a deliberate Play action; debounce and cancel previous play; cap duration and volume; stop when the panel closes. Report missing runtime/dependency errors in the panel.
4. For a cue, audition through the same runtime path as a game call (`audio:play`) so voice policy, inherited effect, fades, and volume match production behavior. For a scene, test immediate/beat/bar transitions against an actual running audio clock. Mark unsupported platform effects and analysis-pending transitions.
5. Source edits trigger refresh via a versioned response. Keep selection by stable ID, discard stale responses, stop or restart audition only with an explicit policy shown to the user. Never silently play unsaved code different from what the UI describes.

## Code-owned sound creation

Start with a **typed, opt-in runtime module**, built on LÖVE `SoundData` and `Source`, not new compiler syntax. A proposed API shape (not implemented):

```tua
local AudioDesign = require("tua.audio.design")

local hit = AudioDesign.render({
  waveform = "noise",
  duration = 0.18,
  envelope = { attack = 0.002, decay = 0.13, sustain = 0, release = 0.04 },
  filter = { type = "lowpass", frequency = 1400 },
  seed = 7,
})

local audio = audioSystem({
  cues = {
    enemyHit = { source = hit, bus = "sfx", maxVoices = 4 },
  },
})
```

`render` would return a reusable static `Source` and render once at load time. Deterministic seeds make output and tests repeatable. Supported waveforms and envelopes need precise parameter bounds, antialiasing/normalization rules, and runtime cost limits. Effects available from LÖVE should stay in `audioSystem` and use LÖVE's supported effect types; a procedural filter in sample generation is a separate offline operation.

For music, first use existing streamed assets, synchronized scene layers, authored BPM, loop regions, and beat/bar transitions. If code-composed tracks are still needed, add a *separate*, typed pattern scheduler over named cues, with explicit tempo, steps/rests, and deterministic event order. Test its scheduling under long frames and scene changes before adding mini-notation. A Strudel-like string language would sacrifice Tua completion and source navigation unless it has its own parser, diagnostics, and mappings, so it is not the first step.

## Implementation slices and acceptance

1. **Truthful model and navigation.** Extend the core/LSP graph, add source spans and provenance, test same-file/imported declarations and dynamic fallbacks. The view can answer “which source, bus, and effect does this cue use?” and open each declaration/asset. No playback yet.
2. **Cue audition and asset view.** Use the actual LÖVE runtime, handle static/streaming sources, error/stop/reload, and compare audible output with a fixture game. Measure click-to-sound and idle CPU/memory; avoid repeated Source construction.
3. **Music scene workbench.** Show authored and analyzed timing separately, layers and loop regions, and simulate actual transitions. Test immediate, beat, bar, analysis-pending, and missing-layer cases.
4. **Procedural sound library.** Implement a small typed runtime helper with generated Lua tests and audible/PCM fixtures. Keep it opt-in by import/reference and document rendering cost. Decide whether to ship only after sound quality and API ergonomics are tested.
5. **Pattern composition research.** Prototype one small soundtrack and one event-driven game. Only then decide whether a typed scheduler adds enough value beyond scenes/layers and raw LÖVE.

For every slice, test empty/loading/error/stale/narrow/light/dark/high-contrast views, keyboard focus and source navigation, real LÖVE audio output, and host performance. Unit snapshots alone cannot establish audible or editor-host behavior.

## Open product decisions

- Whether audition may load arbitrary user module top-level code, or must be restricted to statically extracted declarations. The latter is safer but cannot represent computed Sources; this requires a clear trust and fidelity choice before implementation.
- Whether procedural source rendering should expose only a compact preset-based API or a low-level sample callback. A callback is more flexible but much easier to make slow or non-deterministic.
- Whether game music needs step sequencing at all. Existing layered streamed music may cover the main need with far less scheduling complexity.

## References

- Tua implementation: `docs/audio.md`, `examples/audio-system/src/main.tua`, `crates/tua-core/src/audio.rs`, `editors/vscode/src/audio/index.js`.
- Strudel REPL and live feedback: <https://strudel.cc/technical-manual/repl/>; mini-notation: <https://strudel.cc/learn/mini-notation/>.
- LÖVE Source and SoundData: <https://love2d.org/wiki/Source>, <https://love2d.org/wiki/SoundData>, <https://love2d.org/wiki/love.audio.newSource>.
- LÖVE effects: <https://love2d.org/wiki/love.audio.setEffect>.
