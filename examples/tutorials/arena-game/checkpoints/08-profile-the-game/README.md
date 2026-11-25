# Chapter 8: profile the game

Keeps the cumulative game and adds a bounded wave-size control. On the title screen,
**F6** selects 8, 32 or 128 enemies for the next wave. Eight remains the default. This
is a transient diagnostic setting, not saved difficulty or balanced wave design.
The 128-enemy ring intentionally crowds sprites; it is a workload, not a visual target.
All movement, combat, dialogue, save and presentation controls from chapter 7 remain.

```sh
tua check --deny-warnings
tua build
love build
```

## Inspect and profile

Enable Tua's Runtime Inspector, restart through the extension, and inspect the ECS
world, state machine and bullet pool. A snapshot is bounded and may be truncated.
Use **Tua: Profile LOVE Game** to capture update/draw/frame timings and sampled CPU
stacks. Profiler and inspector code remain in temporary launchers, outside this build.

The example collects only actual attack intents into a reused ID array, then resolves
live components after projectile damage. The former all-enemy attack pass is preserved
in `tests/reference-combat.tua` for a reproducible comparison. The reference is built
as typed source in a temporary project; generated Lua is never patched.

## Repeat the measurements

From repository root, using fresh output paths:

```sh
python3 examples/tutorials/arena-game/checkpoints/08-profile-the-game/tests/measure.py \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --output /tmp/arena-measure
node examples/tutorials/arena-game/checkpoints/08-profile-the-game/tests/profile.js /tmp/arena-profile.json
node examples/tutorials/arena-game/checkpoints/08-profile-the-game/tests/profile.js /tmp/arena-reference-profile.json --baseline
node examples/tutorials/arena-game/checkpoints/08-profile-the-game/tests/inspector.js /tmp/arena-inspector.json
```

Node scripts use the repository compiler and macOS LÖVE path by default; set `TUA_BIN`
and `TUA_LOVE` to select other binaries. Run a fresh `tua build` before profiling the
candidate. `measure.py` builds both implementations itself. It needs Python, LÖVE and
`lua` for the exact per-step differential check. Keep other heavy work idle while
measuring and retain all trials, including unfavorable ones.

The timing workload uses a fixed seed, 120 warm-up updates, 600 measured updates and
five independent trials per variant/size in alternating order. One-second episodes
keep the player alive; assertions reject idle updates. Timers cover simulation update
only, excluding reset/draw/sleep. Separate 60-update GC-stopped probes report allocated
KiB and post-collection retained delta. These are not rendered FPS or platform limits.

Recorded trials showed a small consistent allocation reduction but no reliable CPU
or p95 improvement across batches. Behavior-tree evaluation remained the dominant
sampled cost. See the chapter and qualification report for the full results.

## Regression checks

```sh
sh tests/check.sh
```

The measurement runner also compares 3,600 mixed updates exactly against the frozen
reference, including attacks, deaths, replacement and reset. Run `tests/verify.py`
for the inherited two-process save tests. The native replay runner supports:

| Scenario | Frames |
| --- | ---: |
| `tests/workload-replay.lua` | 45 |
| `tests/replay.lua` | 350 |
| `tests/save-replay.lua` | 40 |
| `tests/dialogue-replay.lua` | 60 |
| `tests/shader-replay.lua` | 50 |

Use `scripts/love_replay.py --project` pointing at this checkpoint, `--scenario` at
the chosen script, `--fps 30 --seed 71`, an unused `--output` directory and `--capture`.
The workload scenario verifies 128-enemy rendering and returning to the default wave.
The inspector script checks native transport; it does not qualify the installed editor UI.

[Read the chapter](../../../../../docs/tutorials/arena-game/profile-the-game.md).
Next: qualify the complete learning path.
