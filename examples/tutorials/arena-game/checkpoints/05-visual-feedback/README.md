# Chapter 5: scoped shader feedback

Extends chapter 4 with a hit-flash shader, split GLSL, initialization before activation,
queued preload, retry and normal-sprite fallback. This checkpoint is self-contained.

## Run

```sh
tua check --deny-warnings
tua build
love build
```

## Controls

All previous movement/combat/menu/audio controls remain. **G** toggles the hit shader;
**L** retries loading. **F** disables all visual hit feedback, including the shader.
**M** removes motion but retains the brief color flash, just as it retains hit rings.

## Expected behavior

Only the most recently hit surviving enemy flashes. Shadows, health bars and HUD do
not use the flash shader. A failed first compile renders normal sprites. A failed
replacement retains the last working shader. The HUD reports queued progress or
availability; detailed errors go to the terminal. Gameplay does not wait for loading.

`HitShader.new(true)` selects the one-entry preload queue; change it to `false` for
synchronous protected loading. The scanner updates even on title/pause screens.
Runtime edits to files under the running `build/shaders` can hot reload, but authored
changes belong in `src/shaders` and should be rebuilt. Do not commit generated Lua.

## Try a change

Change the flash color in `src/shaders/lib/flash.glsl`, build, and run. Temporarily
break that file in a disposable build to observe fallback/last-good retention, then
fix it and press L. Increase the flash duration in `feedback.tua` without changing
combat. A single GPU compile can still block; preload is not a background compiler.

## Verification

```sh
tua fmt --check
tua fmt --check tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/soundscape.lua tests/replay.lua tests/shader-replay.lua
tua check --deny-warnings
tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
lua ../tests/decisions.lua
lua ../tests/soundscape.lua
```

From repo root, use fresh output folders:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/05-visual-feedback \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/05-visual-feedback/tests/shader-replay.lua \
  --frames 50 --fps 30 --seed 71 --output /tmp/arena-shader-replay --capture
```

Also run `tests/replay.lua` through the same runner with 350 frames for the inherited
combat/audio qualification. The shader suite tests real GPU pixel output, draw scope,
callback-error restoration, initial/replace/setup/include failures, retry and disposal.
Its intentional failure messages are expected; the final marker must report success.

## Next steps

[Read the chapter](../../../../../docs/tutorials/arena-game/visual-feedback.md).
Continue with [chapter 6: conversation and language](../06-conversation-and-language/README.md).
