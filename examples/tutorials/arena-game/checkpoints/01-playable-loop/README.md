# Chapter 1: the playable loop

One small game combining input, character animation, camera follow and state-machine
flow. This checkpoint is self-contained; later chapters extend it.

## Run

With Tua and LÖVE installed, from this folder:

```sh
tua check --deny-warnings
tua build
love build
```

Kenney character artwork and its CC0 license are included. No downloads are needed.

## Controls

Enter starts/resumes; WASD or arrows move; Escape pauses/resumes; R restarts an
active run; Backspace returns to title. Close the window to quit. These are keyboard
shortcuts, not pointer-selectable menu buttons. Focus loss pauses an active run;
returning focus does not resume automatically.

## Expected behavior

Title and pause stop movement, animation and camera updates. A transition consumes
its update: holding movement while pressing Enter cannot move until the following
update. Held movement resumes after that; it does not require a new key press.
Restart resets position, animation, facing and camera together. The HUD remains in
screen space. There is no combat, persistence, gravity or controller mapping yet.

## Try a change

Change movement speed in `session.tua`, camera smoothing in `main.tua`, or a key
binding. Keep state gating around all world updates so pause remains consistent.

## Verification

From this folder (substitute your compiler path if needed):

```sh
tua fmt --check
tua fmt --check tests/movement.lua tests/session.lua tests/replay.lua
tua check --deny-warnings
tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
```

From the repository root, using a fresh output folder:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/01-playable-loop \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/01-playable-loop/tests/replay.lua \
  --frames 125 --fps 30 --seed 71 --output /tmp/arena-chapter-one --video
```

## Next steps

[Read the chapter](../../../../../docs/tutorials/arena-game/playable-loop.md).
The next planned checkpoint adds weapons and enemy waves to this same game.
