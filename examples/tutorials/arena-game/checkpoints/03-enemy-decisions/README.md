# Chapter 3: enemies that make decisions

Extends chapter 2 with per-enemy behavior runners, patrol/chase/attack decisions,
player health and game over. This checkpoint is self-contained.

## Run

```sh
tua check --deny-warnings
tua build
love build
```

## Controls

Enter starts/resumes; WASD/arrows move; mouse aims; hold left mouse or Space to fire.
Escape pauses/resumes. R restarts; Backspace returns to title. N starts another wave
once enemies and bullets are gone. F toggles feedback; M toggles reduced motion.
At game over, Enter/R restarts; Escape does not resume a dead player. Close to quit.

## Expected behavior

One shared tree gives each enemy an independent runner. At distance greater than
300 world pixels enemies patrol horizontally around their spawn X; within 300 they
chase at 60 pixels/second. Within 32 they stop and attack, immediately if ready and
then every 120 ticks (one simulated second). You move at 180 pixels/second.

Player health starts at 10. Each accepted attack removes one; there is no invulnerability
window, armor, knockback or avoidance. Enemies can overlap. Health carries between
waves, but restart restores it. Game over freezes simulation and cancels active brains.

Each tick moves the player, evaluates brains/moves enemies, resolves swept projectiles,
then applies surviving enemy attack intents. A lethal shot prevents that enemy's attack
in the same tick. Collision uses relative movement between bullet and enemy, including
an enemy crossing a stationary bullet. World reset/disposal cancels brains before clear.

## Try a change

Change 300 detection distance or 60 chase speed in `brain.tua`. Increase the 120-tick
attack cooldown to make enemies less dangerous. Keep cooldown reduction in the same
fixed simulation clock. Run away to observe chase yielding back to patrol.

## Verification

```sh
tua fmt --check
tua fmt --check tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/replay.lua
tua check --deny-warnings
tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
lua ../tests/decisions.lua
```

`combat.lua` explicitly freezes/removes brains for its retained stationary-target
fixtures; `decisions.lua` and the native replay exercise active brains. From repo root:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/03-enemy-decisions \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/03-enemy-decisions/tests/replay.lua \
  --frames 350 --fps 30 --seed 71 --output /tmp/arena-decisions-replay --video
```

Use a fresh output directory. These are functional checks, not performance benchmarks.

## Next steps

[Read the chapter](../../../../../docs/tutorials/arena-game/enemy-decisions.md).
Continue with [chapter 4: sound](../04-sound/README.md).
