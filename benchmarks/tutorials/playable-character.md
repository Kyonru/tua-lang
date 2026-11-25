# Playable character qualification

Scope: tutorial M1, examples only. Example commit: `8a11c484`.
No compiler or runtime implementation changes.
M0 API/example audit is committed as `f0b169fe`. Qualification used the final
working source following that commit; see the capture provenance for its dirty-tree
status and scenario digest, rather than treating the base revision as final source.

## Provenance

- Compiler: release 0.11.0, embedded revision `e0033fc65d3b78f9be8aba3e427aa9a239782354`,
  target aarch64-apple-darwin; binary SHA-256
  `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
- Runtime: LÖVE 11.5.0, LuaJIT 2.1.1700008891, JIT disabled by the replay runner.
- Headless movement checks: local Lua 5.4.8.
- Native run: 150 frames, fixed 1/30-second steps, seed 71; functional replay,
  not a CPU, allocation or FPS benchmark. No audio is used.
- Kenney Platformer Characters CC0 atlas is unchanged; original archive path,
  download URL and hashes are in the example's `src/assets/provenance.json`.
- Selected screenshot and full capture metadata:
  `docs/assets/tutorials/playable-character/provenance.json`.

## Checks and outcomes

From `examples/tutorials/playable-character`:

```sh
../../../target/release/tua fmt --check
../../../target/release/tua fmt --check tests/movement.lua tests/replay.lua
../../../target/release/tua check --deny-warnings
../../../target/release/tua build
cd build
lua ../tests/movement.lua
```

All passed. Movement covers 30 versus 144 update steps, stopped input and world
bounds. Emission includes Baton, anim8, sprite and camera helpers; it excludes
unused ECS, dialogue, save and behavior-tree runtimes.

From the repository root:

```sh
python3 scripts/love_replay.py --project examples/tutorials/playable-character \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/playable-character/tests/replay.lua \
  --frames 150 --fps 30 --seed 71 --output /tmp/tua-character-qualified --video
```

Passed `CHARACTER_NATIVE_OK`: actual normalized diagonal input, walk/idle
transitions, repeated-play animation continuity, damped following, focus loss,
release/refocus, reset and HUD transform at the point its text is drawn. Inspected
frame 45: plain background, Kenney character, world marker and fixed screen text.

A temporary JSON-RPC LSP session using this compiler/project confirmed hover for
`controls` and `idle`/`walk` completion inside `hero:play`'s string argument. Hover
also remained available after an unsaved input edit. A separate disposable project
changed `controls:get("move")` to `controls:get("unknown")`; `tua check` rejected it
with TL1003. These are protocol/checker tests, not installed-extension visual tests.

Harness corrections during qualification: the replay wraps `love.update` with its
fixed step, so explicit one-second setup probes use the exported original update.
Completion inside the prefix `i` correctly returned only `idle`; the unprefixed
query returned both names. Neither was a product defect. Unsolicited LSP diagnostic
notification timing was not qualified; invalid-name rejection was checked through
the compiler instead.

## Limits

The example is free movement with a side-facing sprite, not four-direction artwork,
physics or collision detection. It deliberately plays walking from input intent at
world boundaries. No browser/Windows/Linux runtime was tested. Timing results are
not performance claims. The small scene has no grid or decorative UI panels.

The README documents the reproducible checks. Public tutorial prose omits this
maintainer-specific qualification detail.
