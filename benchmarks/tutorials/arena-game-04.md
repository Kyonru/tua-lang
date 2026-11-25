# Arena game, chapter 4 qualification

Checkpoint commit: `01cba6ac`. Enemy shadows landed separately in `f26153b7` for the
standalone enemy wave and checkpoints 2/3, then carried into this checkpoint.
No compiler/runtime changes were made.

## Provenance and commands

Compiler: development release 0.11.0, embedded revision
`e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin;
SHA-256 `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
Headless: Lua 5.4.8. Native: LÖVE 11.5.0, LuaJIT 2.1.1700008891, JIT disabled
by the replay runner. These identify the tested binary and host, not all releases.

From the checkpoint directory:

```sh
../../../../../target/release/tua fmt
../../../../../target/release/tua fmt tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/soundscape.lua tests/replay.lua
../../../../../target/release/tua fmt --check
../../../../../target/release/tua fmt --check tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/soundscape.lua tests/replay.lua
../../../../../target/release/tua check --deny-warnings
../../../../../target/release/tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
lua ../tests/decisions.lua
lua ../tests/soundscape.lua
```

From repository root:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/04-sound \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/04-sound/tests/replay.lua \
  --frames 350 --fps 30 --seed 71 --output /tmp/tua-arena-c4-final --video
make docs-build
make json-check
make diff-check
```

Use a fresh replay output folder. The final replay uses 350 fixed-step rendered frames
plus direct-update setup/verification and two 80 ms native-clock observations. Its
metadata is retained in `docs/assets/tutorials/arena-game/04-provenance.json`; source
revision is the pre-commit dirty base, separate from compiler provenance. The video
is silent (`audio_captured: false`). It is not an audio recording or performance test.

## Results

All final formatting, typed, emitted-Lua, native replay and documentation checks
passed. The five Lua suites retain movement/session/combat/decision coverage and add
fake-backend assertions for one-time scene changes, cue coalescing, paused/focused
transport policy, clamped independent gains, preserved mute, counter resets and
idempotent shutdown. Muted audio does not gate gameplay.

Native tests use the actual emitted audio runtime and LÖVE backend. They verify one
native music Source, a frozen Source position during pause and advancing position
after resume, separate bus volume changes, master mute, 30 restarts retaining one
music voice, cue playback and zero active Sources after shutdown. Existing moving
combat, feedback-parity and natural game-over/restart assertions also passed.
`runtime.log` reports `ARENA_AUDIO_NATIVE_OK`, `ARENA_DECISIONS_NATIVE_OK` and
`TUA_REPLAY_OK`.

Two early harness assumptions were corrected: LÖVE 11.5 does not provide
`love.audio.getActiveSources`, and `getActiveSourceCount` includes paused Sources.
The final pause/resume assertion samples the real music Source clock exposed by the
runtime snapshot, rather than equating active count with audible playback. No audio
runtime fix was required.

The final playing view was inspected at 960×640; the replay also runs at 640×480.
Enemy ellipses match the player's shadow and remain world-space decorations. The
shadow-only replay `/tmp/tua-enemy-shadows` passed the chapter 3 native suite; all
three changed shadow projects passed formatting, typed checking and build.

Six bundled original WAVs were generated from the committed Python script. SHA-256
checks match the asset manifest; all files are mono 22050 Hz, non-silent, and below
full-scale clipping. Their procedural origin and CC0 dedication are documented.
No sample buffer is constructed at runtime. Kenney image assets remain unchanged.

## Limits and ownership

Actual native playback/transport was verified; no human listening-quality study or
system-audio capture was performed. The synthetic tones/loops are teaching fixtures,
not a claim of finished sound design. No Windows/Linux or installed-extension/LSP
qualification is claimed.

The adapter stops music before starting the replacement and fades only the new
scene in. It is not a crossfade, beat transition or audio-analysis tutorial. Sound
cues summarize each category once per outer update; damage still counts every hit.
Bus/cue limits may reject cosmetic sound playback without affecting simulation.

The selected audio runtime has no public system-disposal method. The host stops the
master bus and releases its six templates once. Runtime-owned cloned Sources are
stopped and dereferenced, with native reclamation subsequently handled by LÖVE/Lua
GC. Tests establish stopped playback, not immediate native deallocation of every
clone. Volume/mute preferences survive in-game restart but are not yet persisted.
