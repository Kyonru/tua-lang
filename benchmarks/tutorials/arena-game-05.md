# Arena game, chapter 5 qualification

Checkpoint commit: `9f7cb571`. Adds hit-shader presentation and loading ownership;
no runtime/compiler modifications. Earlier checkpoints remain unchanged.

## Provenance and commands

Compiler: development release 0.11.0, embedded revision
`e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin;
SHA-256 `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
Headless Lua 5.4.8; native LÖVE 11.5.0 / LuaJIT 2.1.1700008891 with JIT disabled
by the replay runner. The pinned binary does not represent every 0.11.0 distribution.

From the checkpoint:

```sh
../../../../../target/release/tua fmt
../../../../../target/release/tua fmt tests/shader-replay.lua
../../../../../target/release/tua fmt --check
../../../../../target/release/tua fmt --check tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/soundscape.lua tests/replay.lua tests/shader-replay.lua
../../../../../target/release/tua check --deny-warnings
../../../../../target/release/tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
lua ../tests/decisions.lua
lua ../tests/soundscape.lua
```

From the repository root, with fresh output directories:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/05-visual-feedback \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/05-visual-feedback/tests/replay.lua \
  --frames 350 --fps 30 --seed 71 --output /tmp/tua-arena-c5-game --video
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/05-visual-feedback \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/05-visual-feedback/tests/shader-replay.lua \
  --frames 50 --fps 30 --seed 71 --output /tmp/tua-arena-c5-qualified --capture
make docs-build
make json-check
make diff-check
```

## Results

All final gates passed. Retained emitted-Lua suites cover simulation, lifecycle and
audio policy. The 350-frame native replay verifies integrated gameplay/audio,
feedback parity, game over, restart and cleanup with the shader present.

The 50-frame GPU suite verifies:

- Queued progress 0/1 → 1/1, required uniform presence and synchronous loading.
- Actual Canvas pixel change on a flashed draw and unchanged adjacent draw.
- Previous shader restoration, including a thrown drawing callback.
- Initial compilation failure with ordinary rendering fallback.
- Failed compilation and setup on replacement preserving the last working Shader.
- Missing include diagnostics naming the included file and successful retry.
- Successful replacement, pending-job cancellation, cleared lookup and idempotent disposal.

Failure fixtures intercept native compilation/filesystem reads in the isolated test
process and restore them before continuing; production GLSL and generated Lua are not
edited. Expected error messages precede `ARENA_SHADER_NATIVE_OK` and `TUA_REPLAY_OK`.
The setup callbacks operate on real native Shader objects, and Canvas reads exercise
the actual GPU path rather than a mocked `setShader` implementation.

Frame 5 from the GPU replay was visually inspected and retained as
`docs/assets/tutorials/arena-game/05-shader.png`; its metadata is the adjacent
`05-provenance.json`. Source revision there is the pre-commit base with a dirty
worktree, separate from compiler identity. The selected image shows a flashed
survivor with unchanged shadow, health bar and HUD. Generated Lua/full capture
sequences are not committed. Audio assets and Kenney atlases are carried unchanged.

## Limits

The one-entry preload job demonstrates ownership/progress rather than a multi-entry
loading animation. Compilation still happens on the main thread and may block its
update; first-draw warming is separate and not performed. No GPU timing, allocation,
frame-rate or universal shader support claim is made.

The latest hit is summarized into one surviving-entity flash; simultaneous impacts
are not independently animated. Reduced motion disables recoil/shake but retains
color feedback; F disables all visual hit feedback and G disables the shader alone.
Loading failures do not gate gameplay. Automatic reload uses the existing scanner;
the regression explicitly exercises retries, not filesystem edit-watcher timing.

No Windows/Linux GPU run, installed-extension shader diagnostics/hover test, human
accessibility study or system-audio recording was performed. Native playback tests
are inherited from chapter 4; the MP4 remains silent. The controller stops updating
the scanner after cancelling its job and releasing the active shader at shutdown.
