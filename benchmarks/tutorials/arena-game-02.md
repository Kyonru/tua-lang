# Arena game, chapter 2 qualification

Checkpoint commit: `505a65ea`. Extends chapter 1 without changing that checkpoint.
No compiler/runtime changes. Standalone M4 and M5 research evidence is not substituted
for this integrated qualification.

## Provenance

- Compiler: development release 0.11.0, embedded revision
  `e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin.
- Compiler SHA-256: `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
- Emitted-module regressions: Lua 5.4.8.
- Native: LÖVE 11.5.0, LuaJIT 2.1.1700008891, JIT disabled by the runner.
- Final replay: `/tmp/tua-arena-c2-qualified`, seed 71, 125 rendered frames at 30 Hz,
  plus deterministic direct-update trials during setup and verification.
- Metadata: `docs/assets/tutorials/arena-game/02-provenance.json`; its source revision
  is the pre-commit base with a dirty tree, not the final checkpoint commit.
  Compiler provenance is recorded independently.

## Commands

From `examples/tutorials/arena-game/checkpoints/02-combat-waves`:

```sh
../../../../../target/release/tua fmt
../../../../../target/release/tua fmt tests/movement.lua tests/session.lua tests/combat.lua tests/replay.lua
../../../../../target/release/tua fmt --check
../../../../../target/release/tua fmt --check tests/movement.lua tests/session.lua tests/combat.lua tests/replay.lua
../../../../../target/release/tua check --deny-warnings
../../../../../target/release/tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
```

From repository root, with a fresh output folder:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/02-combat-waves \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/02-combat-waves/tests/replay.lua \
  --frames 125 --fps 30 --seed 71 --output /tmp/tua-arena-c2-qualified --video
make docs-build
make json-check
make diff-check
```

## Results

All final commands passed. Headless checks cover movement bounds, 100 session cycles,
missed-shot cadence/outcome parity at 30/60/120/144 Hz, exhaustion, expiry, swept
crossings in both directions, starting inside, stationary/missing segments,
deterministic coincident-target ties, component replacement, destroyed-target
rejection, cleared pooled fields/reuse, 100 combat resets, next-wave gates,
bounded stall catch-up and idempotent disposal.

Native replay verifies actual input polling and world-space aim, feedback on/off/
reduced-motion hit and defeat parity, paused projectiles/animation, focus return,
reset with live combat, wave clear, next wave and title freezing. Selected captures
were inspected at 960×640 and 640×480. Opaque HUD strips were added after observing
sprites overlapping labels in the smaller capture. The view remains world-sized;
smaller windows crop more of the world and movement reveals offscreen enemies.

The first native attempt failed because the test used the OS mouse setter while
the runner supplies a virtual pointer. The scenario now uses `context.pointer`;
this was a harness error, not a collision or camera fix. Final runtime log reports
`ARENA_COMBAT_NATIVE_OK` and `TUA_REPLAY_OK`.

The emitted runtime includes ECS, object pooling, tweening, camera, sprites, input
and state machines. No dialogue/audio runtime is introduced. Generated Lua is ignored;
source remains typed. Both Kenney atlases have bundled CC0 notices and provenance.

## Limits

This is functional qualification, not a CPU/FPS or allocation benchmark. Cadence
parity uses identical held input and simulated time; pointer sampling differences
and the documented 250 ms stall clamp are not claimed wall-clock equivalent.
The eight-enemy/eight-projectile bounds are teaching policy, not engine limits.
Damage is one point per accepted projectile; there is no gameplay reward economy.

Enemies are stationary and do not attack. Collision is segment versus static circles,
not moving-target continuous collision. Structural changes occur only at documented
phase boundaries with no external callbacks during target selection. Camera feedback
is a drawing-only offset; targeting uses the unshaken camera. Multiple hits within
an outer update share the latest cosmetic impact indicator.

No Windows/Linux native run, installed-extension/LSP qualification, human usability
study or controller input test was performed. UI reference direction continues from
chapter 1; no new proprietary reference artwork is included.
