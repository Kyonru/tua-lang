# Arena game, chapter 1 qualification

Checkpoint commit: `8de8b80b`. Scope: cumulative tutorial example and documentation;
no compiler/runtime changes. Existing standalone examples remain untouched.

## Provenance

Compiler: development release 0.11.0, embedded revision
`e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin;
SHA-256 `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
This is the selected binary, not a promise about every 0.11.0 distribution.

Headless tests ran on Lua 5.4.8. Native replay used LÖVE 11.5.0 and LuaJIT
2.1.1700008891 with JIT disabled by the replay runner. Final replay:
`/tmp/tua-arena-c1-final`, seed 71, 125 frames at fixed 1/30-second updates.
`docs/assets/tutorials/arena-game/01-provenance.json` records complete provenance;
its repository revision is the pre-commit base with a dirty tree. Compiler provenance
is separate. This is functional qualification, not a performance measurement.

## Checks and results

From `examples/tutorials/arena-game/checkpoints/01-playable-loop`:

```sh
../../../../../target/release/tua fmt
../../../../../target/release/tua fmt tests/movement.lua tests/session.lua tests/replay.lua
../../../../../target/release/tua fmt --check
../../../../../target/release/tua fmt --check tests/movement.lua tests/session.lua tests/replay.lua
../../../../../target/release/tua check --deny-warnings
../../../../../target/release/tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
```

Passed: formatting, warning-free checking, emitted build, movement at different
step sizes/bounds and 100 title/start/pause/resume/focus/restart/leave cycles.
Transition frames never advance the simulation. Focus regain alone never resumes.

From the repository root:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/01-playable-loop \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/01-playable-loop/tests/replay.lua \
  --frames 125 --fps 30 --seed 71 --output /tmp/tua-arena-c1-final --capture
make docs-build
make json-check
make diff-check
```

Passed native callbacks/input polling, transition-frame ownership, character movement,
paused animation/camera freeze, explicit resume after focus, reset of presentation,
return to title and HUD transform assertions. Inspected playing/paused captures at
960×640 and title at 640×480. The final replay changes window size at frame 100.
The earlier identical-size replay also produced a local video at `/tmp/tua-arena-c1`.
Generated Lua and full capture sequences are not committed.

## Visual reference and limits

Artstyle reference `ui-1237950-29`, STAR WARS Battlefront II, credited to DICE:
[Artstyle](https://artstyle.games/interfaces?q=STAR%20WARS%20Battlefront%20II),
[original source](https://interfaceingame.com/screenshots/star-wars-battlefront-ii-pause-menu/).
Inspected the preview: visible gameplay behind a dim overlay, compact left action
list. Adapted that hierarchy with text shortcuts and existing Kenney artwork;
no proprietary images/assets were copied. Reference copyright belongs to its
respective rights holders and permits reference use only. No animation or behavior
was inferred from the static screenshot.

The chapter uses keyboard shortcuts, not pointer-selectable menus; no controller
mapping, audio, combat, save system or platformer physics is claimed. The world
landmarks are plain teaching primitives, not finished harbor artwork. The current
pause policy retains held movement, which resumes on the next ordinary playing
update after a transition. No Windows/Linux native run, installed-extension test,
new LSP semantic qualification or human usability study was performed.
