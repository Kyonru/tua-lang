# Chapter 7: save progress

Extends chapter 6 with a typed between-wave checkpoint and persistent preferences.
Live combat, dialogue and native resource objects are recreated, never serialized.

## Run

```sh
tua check --deny-warnings
tua build
love build
```

Keep the chapter 6 controls. Clear a wave and let all bullets expire to save its
number and remaining health automatically. N opens the next-wave conversation.
On the title screen, **F9** resumes the saved empty boundary; N then opens dialogue.
**Enter/R** still start/restart a fresh run, preserving the old save until another
wave is cleared. The latest cleared boundary replaces the previous one, even when
its wave number is lower. This is a checkpoint, not a high-score record.

Audio gains, mute, locale, feedback, reduced motion and shader preference autosave
after one quiet second (at most ten seconds while settings keep changing). They can
save during combat without recording current enemies or bullets. **F5** retries a
pending write; quit flushes pending data. A failed write retains in-memory progress.

On the title screen, **F8** explicitly accepts a recoverable copy, which may be older.
Other load issues keep writes blocked. **Delete** requests deletion of both save
copies; **Enter** confirms and **Escape** cancels. Reset failure stays visible and
blocked. Starting a new run is not implicit permission to overwrite a damaged save.

The stable LÖVE identity is `tua-arena-tutorial`, with slot `arena`. This checkpoint
uses its own identity; it does not read any standalone example or zombie-game save.

## Verification

From this folder, using the repository compiler:

```sh
sh tests/check.sh
```

From the repository root, with fresh output directories:

```sh
python3 examples/tutorials/arena-game/checkpoints/07-save-progress/tests/verify.py \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --output /tmp/arena-save-processes
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/07-save-progress \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/07-save-progress/tests/save-replay.lua \
  --frames 40 --fps 30 --seed 71 --output /tmp/arena-save-replay --capture
```

The persistence runner uses two independent LÖVE processes and a unique save identity.
It tests migration, invalid historical input, future versions, domain constraints,
corruption, explicit recovery, partial reset failure, debounce, quit and write retry.
Native replay checks the bullet boundary, preferences, title resume and reset input.
It clears enemies through combat lifecycle APIs to reach the boundary. Retain the
inherited gameplay (350 frames), dialogue (60) and shader (50) scenarios too.

[Read the chapter](../../../../../docs/tutorials/arena-game/save-progress.md).
Next: profile this same game with reproducible workloads.
