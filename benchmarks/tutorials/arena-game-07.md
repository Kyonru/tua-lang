# Arena game, chapter 7 qualification

Checkpoint commit: `c9c0c667`. Adds between-wave save boundaries and persistent
preferences to the cumulative game. No compiler/runtime changes. Earlier checkpoint
folders remain unchanged and the new folder owns its source, tests and assets.

## Provenance

Compiler: development release 0.11.0, embedded revision
`e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin;
SHA-256 `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
This selected binary is separate from repository HEAD and does not represent every
0.11.0 distribution. Headless suites used Lua 5.4.8. Native runs used LÖVE 11.5.0 /
LuaJIT 2.1.1700008891 with JIT disabled by the functional runners.

`arena-game-07-persistence.json` records the two-process runtime/compiler/test identity.
`docs/assets/tutorials/arena-game/07-provenance.json` records the rendered replay,
including its pre-commit dirty source base separately from compiler identity.
The runners use unique save identities, never the playable game's stable identity.

## Commands

From the checkpoint:

```sh
../../../../../target/release/tua fmt
../../../../../target/release/tua fmt src/locales/en.lua src/locales/fr.lua tests/*.lua
sh tests/check.sh
```

`check.sh` checks typed and explicit Lua formatting, runs warning-free type checking,
builds emitted Lua, then runs movement, session, combat, decisions, soundscape and
conversation regressions. Generated output is ignored and was not edited or committed.

From repository root, using fresh output directories:

```sh
python3 examples/tutorials/arena-game/checkpoints/07-save-progress/tests/verify.py \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --output /tmp/tua-arena-c7-qualified-persistence
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/07-save-progress \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/07-save-progress/tests/save-replay.lua \
  --frames 40 --fps 30 --seed 71 --output /tmp/tua-arena-c7-qualified --capture
```

Also ran the same replay command with these scenario/frame/output substitutions:

| Scenario | Frames | Output | Capture option |
| --- | ---: | --- | --- |
| `tests/replay.lua` | 350 | `/tmp/tua-arena-c7-game` | `--video` |
| `tests/dialogue-replay.lua` | 60 | `/tmp/tua-arena-c7-dialogue` | `--capture` |
| `tests/shader-replay.lua` | 50 | `/tmp/tua-arena-c7-shaders` | `--capture` |

## Results

All formatting, type/build, six retained headless suites, two-process persistence
and four native scenarios passed on final source. The persistence suite verifies:

- A genuinely separate process loads cleared wave 4, health 7 and all preferences.
- Restoring creates an empty world with reset simulation/player state; the next wave
  contains eight enemies. Restore is refused outside the title.
- An untouched new profile writes nothing. Preference changes debounce, unchanged
  preferences/UI do not dirty the save, and quit flushes pending settings.
- Valid v1 data migrates without an immediate write, and quit commits its pending
  upgrade. Malformed historical input is rejected before migration.
- Future versions, malformed files and invalid game-domain values block writes.
- A damaged newer copy requires explicit recovery acceptance before writing.
- Reset requires confirmation; failed and partial deletion remain blocked.
- A failed write retains the checkpoint in memory and explicit retry succeeds.

The initial migration test incorrectly expected a quit flush to leave v1 bytes
unchanged. Inspection confirmed that `Save.bind` marks a successful migration dirty
through `pendingUpgrade`; the test now distinguishes read-only loading from the later
intentional upgrade. No production runtime fix was required.

The 40-frame native host scenario keeps a missed bullet alive after clearing enemies
and verifies that no boundary is saved until it disappears. It then checks preferences,
next-wave dialogue, title resume, cancellation and confirmed reset without Enter
leaking into gameplay. Enemy removal in this scenario is a lifecycle fixture, not a
claim that the replay shot through an entire wave. The retained gameplay scenario
covers combat, focus, restart, audio and shutdown; the dialogue and shader scenarios
retain their earlier lifecycle and native GPU checks. Injected shader failures are
expected and followed by successful recovery markers.

Frames 10 and 25 were inspected at 640×480 and retained as `07-save.png` and
`07-reset.png`. French text, controls and confirmation fit. Kenney and audio assets
are unchanged from earlier chapters. The save UI stays on the title; gameplay write
failures replace the shader status row with retry/recovery guidance.

A stdio LSP probe on the actual imported `progress.snapshot()` value returned all
nine profile fields in completion and `(field) health: number` in hover. A disposable
project assigning `progress.snapshot().locale` to a number failed with TL1003.
This is protocol/type-checker evidence, not installed-extension UI qualification.

Documentation verification: `make docs-build`, `make json-check`, `make diff-check`,
and staged whitespace checks. Captured JSON files were explicitly parsed too.

## Limits

The save is the latest cleared boundary, not a high score or live-combat snapshot.
Starting a fresh run preserves the old boundary until a new one is cleared; a lower
wave can replace a higher one. Health persists but player position, enemy identities,
projectiles, counters, dialogue visits, Sources and Shaders do not. Preferences may
save during combat while retaining the previous boundary. Version 1 is an intentional
historical fixture, not an assertion that an earlier arena chapter shipped saves.

The binding writes synchronously. No latency, allocation, FPS, cloud synchronization,
concurrent-writer, power-loss or anti-cheat guarantee is claimed. Testing covered
macOS/LÖVE and the two stated interpreters, not Windows/Linux, arbitrary international
fonts, controllers or a human usability study. Native audio assertions inspect Source
behavior; captures are silent. Full compiler/extension suites were not rerun for this
example-only change. Profiling remains C8 work.
