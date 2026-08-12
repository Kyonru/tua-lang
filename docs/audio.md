---
icon: lucide/audio-lines
---

# Audio Systems

`audioSystem({...})` is Tua's compiler-aware audio layer for LÖVE. It manages
polyphonic cues, hierarchical control buses, priorities, fades, music scenes,
rhythm events, effects, positional sources, and development snapshots while
leaving `sound(...)`, `music(...)`, and raw `love.audio` available.

```tua
local audio = audioSystem({
  maxVoices = 64,
  buses = {
    master = { volume = 1 },
    music = { parent = "master", maxVoices = 8 },
    sfx = { parent = "master", maxVoices = 32 },
  },
  cues = {
    jump = {
      source = sound("assets/jump.wav"),
      bus = "sfx",
      priority = 40,
      maxVoices = 4,
      overflow = "steal",
      fadeIn = 0.02,
    },
  },
  music = {
    exploration = {
      analysis = "assets/exploration.ogg",
      signature = { 4, 4 },
      layers = {
        base = { source = music("assets/exploration.ogg") },
        drums = { source = music("assets/exploration-drums.ogg"), volume = 0 },
      },
    },
  },
  analysis = require("generated.audio.catalog"),
})

function love.update(dt: number)
  audio:update(dt)
end
```

Tua derives exact cue, bus, scene, and layer names from a static declaration.
Misspelled names are type errors and editor completion only proposes configured
names. A local runtime binding named `audioSystem` takes precedence over the
built-in, following the same replacement rule as other Tua helpers.

## Voices, Limits, And Fades

`play` returns an `AudioVoice | nil` and `"started"`, `"stolen"`, or
`"rejected"`. Limits apply globally and at bus and cue level. When a constrained
group is full, `steal` selects its lowest-priority, oldest voice. An incoming
voice cannot steal a higher-priority voice. `overflow = "reject"` can be set at
system, bus, or cue level; the nearest setting wins.

Voice, cue, bus, and ancestor-bus gains multiply. Fades accept seconds or
`{ duration = 0.25, curve = "smoothstep" }`; available curves are `linear`,
`smoothstep`, and `exponential`. Voice handles are generation checked, so a
handle cannot mutate a pooled record after that record has been reused.

Use `pauseBus`, `resumeBus`, `stopBus`, `setBusVolume`, and `setBusMuted` for
groups. `snapshot()` returns detached counts, effective bus gains, fades,
music state, analysis state, and started/stolen/rejected counters. `subscribe`
observes control-state changes. Typed `on` and `once` cover voice lifecycle,
bus changes, fades, music transitions, beats, bars, effects, and analysis.

## Music Scenes

Scenes contain synchronized layers. `setMusicState` accepts an immediate,
beat, or bar transition and an optional fade:

```tua
audio:setMusicState("combat", { quantize = "bar", fade = 0.5 })
audio:setMusicLayer("drums", 1, { duration = 1, curve = "smoothstep" })
```

The leading Source's `tell("seconds")` value is the music clock. The runtime
emits every skipped beat and bar after a long frame and seeks other layers only
when their drift exceeds `driftTolerance` (0.03 seconds by default). Different
tempo scenes leave on the current grid and start on the destination's grid;
Tua does not pitch-shift or time-stretch music. A quantized transition returns
`"analysis-pending"` if neither authored nor sufficiently analyzed timing is
available. Detected timing must meet `analysisConfidence` (0.35 by default).
Use `loopRegion = { start = 8, finish = 40 }` for a sample-position loop.

## Effects And Positioning

Named effects call `love.audio.setEffect` and are namespaced per audio-system
instance. LÖVE does not expose DSP mixer buses, so bus effects are inherited
and applied to each Source; this is control-bus behavior rather than a native
mixer graph. Unsupported effects emit `effectUnsupported` and other playback
continues.

Pass `position`, `velocity`, `direction`, `attenuation`, `cone`, or `relative`
to `play`. LÖVE positional playback only works for mono sources. The built-in
analyzer reports stereo files so this mistake is visible before runtime.

## Cached Audio Analysis

### Built-In Analyzer

Use the built-in content-addressed importer when Tua should decode the audio
and detect its beat automatically:

```toml
[[asset_pipelines]]
name = "audio"
builtin = "audio-analysis"
inputs = ["assets/audio/**/*.wav", "assets/audio/**/*.ogg", "assets/audio/**/*.mp3", "assets/audio/**/*.flac"]
output = "src/generated/audio"
version = "1"

[asset_pipelines.options]
envelope_hz = 20
tempo_min = 60
tempo_max = 200
confidence_warning = 0.35
```

Run `tua check`, `tua build`, or `tua watch`. Analysis happens before the game
is checked, and Tua materializes:

```text
src/generated/audio/catalog.tua
src/generated/audio/catalog_data.lua
src/generated/audio/audio-index.json
```

### Generated Files And Runtime Cost

The three files serve different consumers:

| File | Consumer | Purpose | Included in game build |
| --- | --- | --- | --- |
| `catalog.tua` | Tua compiler and game | Small typed facade. It gives catalog entries completion and checking, then emits a Lua module that loads the runtime data. | Yes, as `catalog.lua` |
| `catalog_data.lua` | Game runtime | Immutable analysis records and numeric envelopes. `require("generated.audio.catalog")` loads this table. | Yes |
| `audio-index.json` | Diagnostics and VS Code Audio Explorer | Editor-friendly duplicate with paths, warnings, candidates, and preview data. Lua never imports it. | No |

Consequently, the JSON index does not affect Lua startup or runtime memory. It
is materialized under the source tree so editor tooling can refresh without a
live game connection, but Tua excludes it when building the game.

`catalog_data.lua` is currently eager: the first `require` parses and creates
the complete catalog table. At the default `envelope_hz = 20`, each analyzed
second contributes roughly 100 envelope values across broadband, low, mid,
high, and onset arrays, in addition to its beat grid. This is small for a few
tracks but can noticeably increase source size, startup parsing, and memory for
large sound libraries.

To keep the initial implementation predictable:

- restrict `inputs` to audio that actually needs analysis;
- lower `envelope_hz` when coarse gameplay energy is sufficient;
- use separate pipelines/catalogs for independently loaded game areas; or
- use a command analyzer that emits timing-only records when only `bpm`,
  `confidence`, and `beatOffset` are needed.

Lazy per-track envelope modules or a compact binary analysis pack would be a
valuable future optimization. They are not implied by the current facade: the
built-in `catalog_data.lua` remains intentionally plain, replaceable Lua.

Pass the generated catalog to `audioSystem` and set the scene's `analysis` to
the exact project-relative key stored in that catalog:

```tua
local analysis = require("generated.audio.catalog")

local audio = audioSystem({
  analysis = analysis,
  music = {
    sample = {
      analysis = "assets/audio/sample.wav",
      signature = { 4, 4 },
      layers = {
        track = { source = music("assets/audio/sample.wav") },
      },
    },
  },
})

audio:on("beat", function(event)
  print("beat", event.beat)
end)

audio:setMusicState("sample")
```

Authored `bpm` and `offset` take precedence. Without them, the scene uses the
detected `bpm` and `beatOffset` when `confidence` meets the system's
`analysisConfidence` threshold. Immediate playback does not require reliable
timing, but beat- and bar-quantized transitions do.

The `examples/audio-system` project demonstrates the complete flow with
`src/sample.wav`. Its catalog key is `src/sample.wav`, while LÖVE loads the
copied runtime asset as `sample.wav`.

The built-in importer uses bundled pure-Rust codecs and produces `catalog.tua`,
immutable `catalog_data.lua`, and `audio-index.json`. Results contain duration,
channel count, sample rate, RMS/peak, broadband and approximate low/mid/high
envelopes, onset strength, BPM, confidence, ranked tempo candidates, metrical
ambiguity, boundary influence, beat offset, and a beat grid. The pipeline's
content hash includes its options and audio bytes. Tua also includes the
bundled analyzer version, so analyzer upgrades invalidate stale catalogs
without requiring a manual pipeline `version` bump.

Tempo analysis cannot always distinguish the musical quarter-note pulse from
half-time, double-time, or another strong subdivision. `tempoCandidates` keeps
up to five local correlation peaks and labels common relationships to the
selected tempo. `tempoAmbiguity` rises when a competing metrical candidate is
nearly as strong. `boundaryInfluence` reports how much the winning correlation
depends on the beginning or end of the file; edge-dominated candidates are
penalized before selection. Confidence incorporates all three signals and is
therefore confidence in the selected interpretation, not merely the presence
of a periodic pulse.

```tua
for _, candidate in ipairs(analysis["assets/audio/sample.wav"].tempoCandidates) do
  print(candidate.bpm, candidate.relativeScore, candidate.relation)
end
```

Authored scene timing remains authoritative when musical intent is known:

```tua
sample = {
  analysis = "assets/audio/sample.wav",
  bpm = 90,
  offset = 0.12,
  signature = { 4, 4 },
  layers = { track = { source = music("assets/audio/sample.wav") } },
}
```

### Command Analyzer

Use `command` for a different decoder or beat tracker, proprietary metadata,
or an existing studio pipeline. A command pipeline does not invoke Tua's
built-in analyzer; the external program owns analysis and generated output.

```toml
[[asset_pipelines]]
name = "audio"
command = [
  "python3",
  "tools/analyze_audio.py",
  "--output", "{output}",
  "--manifest", "{manifest}",
  "{inputs}",
]
inputs = [
  "assets/audio/**/*.wav",
  "assets/audio/**/*.ogg",
  "tools/analyze_audio.py",
]
output = "src/generated/audio"
version = "my-analyzer-v1"
```

The command receives the substitutions and environment variables documented in
[Asset Pipelines](asset-pipelines.md#importer-environment). It must write every
generated file below `{output}` and a version-1 manifest at `{manifest}`. To
feed the result directly into `audioSystem`, emit a catalog with the built-in
runtime shape:

```tua title="generated/audio/catalog.tua"
export type AudioAnalysis = {
  path: string,
  bpm: number | nil,
  confidence: number,
  beatOffset: number,
  beatGrid: number[],
  energy: number[],
}

local catalog: { [string]: AudioAnalysis } = require("generated.audio.catalog_data")
return catalog
```

At minimum, automatic rhythm requires `bpm`, `confidence`, and `beatOffset` for
each project-relative asset key. `beatGrid` and energy arrays provide richer
gameplay and visualization data. Include the analyzer script in `inputs` and
bump `version` when hidden analyzer behavior changes so the cache invalidates.

`builtin` and `command` are mutually exclusive. Configure exactly one per
pipeline.

Detected timing is advisory. Author `bpm`, `offset`, and `signature` when a
transition must be musically exact. Low-confidence estimates produce a
diagnostic. Energy envelopes are intended for gameplay and visualization, not
sample-accurate DSP.

`requestAnalysis(path, options)` is an explicit fallback for dynamic files. It
decodes and scans on a LÖVE worker thread, delivers the result during `update`,
and caches it under LÖVE's save directory by file hash and analyzer version.
It is never started implicitly.

## Runtime Replacement And Explorer

Use **Tua: Show Audio Explorer** to inspect static buses, cues, scenes, layers,
and effects and navigate to their declaration. The explorer deliberately does
not connect to a running game; use `snapshot()` for runtime instrumentation.

`audioSystem` lowers to `require("tuaaudio")`. Builds emit `tuaaudio.lua` plus
its signal and store dependencies only when used and never overwrite files
already supplied by the project. Replacing those files provides the same
plain-Lua runtime customization boundary as Tua's other helper runtimes.
