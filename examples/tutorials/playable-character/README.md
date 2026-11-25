# Your first playable character

Move a Kenney character with typed input, switch between idle/walk poses, and follow
it with a camera. The scene is deliberately plain: a flat background, a starting
marker and two landmarks. The HUD stays in screen space.

## Run

Use LÖVE 11.5 and a current Tua compiler. From the repository root:

```sh
cd examples/tutorials/playable-character
tua check
tua build
love build
```

## Controls

WASD or arrow keys move; R resets; Escape quits. Losing focus stops movement and
selects idle. On refocus, keys still held resume movement; released keys stay stopped.

## Expected behavior

Speed is 180 world pixels/second in every direction. The input pair already
normalizes diagonals. The camera catches up smoothly with rate 6; the heading and
controls remain fixed. Movement is clamped to the small world; walking is selected
from input intent, so pushing against the boundary still plays the walk animation.

`src/main.tua` owns input, resources, animation and camera. `src/movement.tua` is a
small pure movement function. The image is the unchanged Kenney Platformer Characters
adventurer atlas; its CC0 license and hashes are in `src/assets/`. No downloads are
needed to run the example. This is free movement with a side-facing sprite, not a
platformer or a four-direction animation set.

## Try a change

- Set `speed` to 90: travel takes twice as long without changing animation timing.
- Change walk `fps` from 8 to 4: footsteps slow down without changing movement speed.
- Set camera `smoothing` to 0: following snaps immediately. Restore 6 for damping.

## Verification

```sh
tua fmt --check
tua fmt --check tests/movement.lua tests/replay.lua
tua check --deny-warnings
tua build
cd build
lua ../tests/movement.lua
```

From the repository root (adjust executable paths for your system):

```sh
python3 scripts/love_replay.py --project examples/tutorials/playable-character \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/playable-character/tests/replay.lua \
  --frames 150 --fps 30 --seed 71 --output /tmp/tua-character --video
```

The native scenario checks actual input-pair normalization, animation changes,
following, focus loss, reset and the transform used to draw the HUD. The pure
movement test compares 30 and 144 steps and checks boundaries. Captures are functional
checks, not frame-rate measurements. Generated output is not committed.

## Next steps

Read the [integrated game chapter](../../../docs/tutorials/arena-game/playable-loop.md), then explore
[input](../../../docs/input.md), [sprites](../../../docs/sprites.md), or
[camera](../../../docs/camera.md) independently. Combat and ECS are not required here.
