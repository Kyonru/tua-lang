# Chapter 2: combat and waves

This checkpoint extends chapter 1 with aimed projectiles, live ECS enemy health,
bounded reuse, optional recoil/impact feedback and explicit next-wave scheduling.
It is self-contained: copy this folder without any other checkpoint.

## Run

```sh
tua check --deny-warnings
tua build
love build
```

## Controls

- Enter: start/resume. Escape: pause/resume.
- WASD/arrows: move. Mouse: aim. Hold left mouse or Space: fire.
- N: next wave, after enemies and projectiles are both gone.
- R: restart. Backspace: title. Close the window to quit.
- F: toggle feedback. M: reduced motion (no recoil/shake, keep the hit indicator).

## Expected behavior

Eight stationary enemies take three hits each. Bullets travel at 1800 world pixels
per second and expire after 1.2 seconds. The 120 Hz simulation attempts one shot
every 15 ticks (eight per second while held). At eight active bullets, a firing slot
is skipped, without stealing an existing bullet. Aim at a gap to observe exhaustion.
Enemies start around the harbor center, not around the player's current location.
Move to see enemies outside the view; smaller windows show less of the world. The
HUD uses opaque strips so sprites cannot obscure text.

Pause freezes movement, projectiles, animation, camera and feedback. Held firing
resumes on the next ordinary playing update after explicit resume; restart consumes
its frame and clears bullets, enemy IDs, feedback and counters. Settings survive
restart. Updates discard wall time beyond 250 ms rather than catch up indefinitely.
This is a bounded catch-up policy, not wall-clock-perfect shooting after stalls.
Enemies do not move or attack yet. There is no audio, collision with scenery or save.

## Try a change

Change enemy health, projectile speed or the 15-tick cadence in `combat.tua`.
Keep the segment test when changing speed, and preserve the release paths.
Toggle F/M while shooting: damage and trajectory do not change.

## Verification

```sh
tua fmt --check
tua fmt --check tests/movement.lua tests/session.lua tests/combat.lua tests/replay.lua
tua check --deny-warnings
tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
```

Native verification from the repository root (fresh output directory):

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/02-combat-waves \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/02-combat-waves/tests/replay.lua \
  --frames 125 --fps 30 --seed 71 --output /tmp/arena-combat-replay --video
```

## Next steps

[Read the chapter](../../../../../docs/tutorials/arena-game/combat-waves.md).
Chapter 3 will give these enemies decisions, movement and attacks. The unchanged
Kenney atlases, license and provenance are bundled under `src/assets`.
