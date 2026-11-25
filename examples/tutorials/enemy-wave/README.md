# A wave of enemies

Eight Kenney enemies, live ECS health, click-to-damage targeting and explicit wave
boundaries. Defeat rings are optional and start disabled. No external downloads.

## Run

From this directory, using a Tua compiler with the current ECS reader API:

```sh
tua check --deny-warnings
tua build
love build
```

## Controls

- Left click: damage the nearest enemy within 30 pixels of its center.
- Space: start the next eight-enemy wave, only after the current wave is empty.
- R: reset to wave one and clear active effects.
- E: toggle cosmetic defeat rings (initially off).
- Escape: quit.

## Expected behavior

Enemies take two or three hits. Their health bars shrink before they disappear.
Defeated entities stop moving immediately and cannot award a second defeat. There
is no automatic spawning, difficulty ramp, player damage or persistence. Rings
expire after 0.4 seconds and never keep an entity or component reference.

## Try a change

Change the horizontal speed in `src/wave.tua`, then change initial health. Keep
spawning outside query callbacks. Enable rings only after understanding the ECS
lifecycle; the pool is bounded by host policy to 16 active rings.

## Verification

```sh
tua fmt --check
tua fmt --check tests/lifecycle.lua tests/replay.lua
tua check --deny-warnings
tua build
cd build
lua ../tests/lifecycle.lua
```

From the repository root, run the actual LÖVE callbacks and renderer:

```sh
python3 scripts/love_replay.py --project examples/tutorials/enemy-wave \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/enemy-wave/tests/replay.lua \
  --frames 150 --fps 30 --seed 71 --output /tmp/enemy-wave-replay --video
```

Use a fresh output directory. This is a functional replay, not a performance test.

## Next steps

Read [the integrated game chapter](../../../docs/tutorials/arena-game/combat-waves.md) and
[qualification results](../../../benchmarks/tutorials/enemy-wave.md).
The arena chapter adds weapons and projectile collision. This focused example
uses clicks to apply damage.
