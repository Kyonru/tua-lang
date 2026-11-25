# Arena game, chapter 6 qualification

Checkpoint commit: `5eef1d6a`. Adds a localized between-wave conversation to chapter
5. No compiler or runtime changes; earlier checkpoints remain unchanged.

## Provenance and commands

Compiler: development release 0.11.0, embedded revision
`e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin;
SHA-256 `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
Headless Lua 5.4.8; native LÖVE 11.5.0 / LuaJIT 2.1.1700008891 with JIT disabled
by the replay runner. This pinned development binary is separate from source HEAD
and does not represent every 0.11.0 distribution.

From `examples/tutorials/arena-game/checkpoints/06-conversation-and-language`:

```sh
../../../../../target/release/tua fmt
../../../../../target/release/tua fmt src/locales/en.lua src/locales/fr.lua
../../../../../target/release/tua fmt --check
../../../../../target/release/tua fmt --check src/locales/en.lua src/locales/fr.lua tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/soundscape.lua tests/replay.lua tests/shader-replay.lua tests/conversation.lua tests/dialogue-replay.lua
../../../../../target/release/tua check --deny-warnings
../../../../../target/release/tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
lua ../tests/decisions.lua
lua ../tests/soundscape.lua
lua ../tests/conversation.lua
```

From the repository root, with fresh output directories:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/06-conversation-and-language \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/06-conversation-and-language/tests/dialogue-replay.lua \
  --frames 60 --fps 30 --seed 71 --output /tmp/tua-arena-c6-qualified --capture
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/06-conversation-and-language \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/06-conversation-and-language/tests/replay.lua \
  --frames 350 --fps 30 --seed 71 --output /tmp/tua-arena-c6-game --video
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/06-conversation-and-language \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/06-conversation-and-language/tests/shader-replay.lua \
  --frames 50 --fps 30 --seed 71 --output /tmp/tua-arena-c6-shaders --capture
make docs-build
make json-check
make diff-check
```

## Results

Source and explicit Lua formatting, warning-free checking, emitted build, and all
six headless suites passed. The conversation suite verifies stable visit/generation
across repeated locale refreshes, localized speaker/text, valid initial choice focus,
selection retention, stable reply IDs, cancellation and one-shot wave requests.
It also checks named interpolation, parent-locale resolution, authored-text fallback
and disposal. A missing French entry is injected in memory before loading I18n in
that isolated test process; shipped catalogs are complete.

A disposable project containing `I18n.t("ui.thisKeyDoesNotExist")` failed
`check --deny-warnings` with TL5010 as expected. An initially omitted French entry
also produced diagnostics; it was restored before final qualification. Fallback
coverage does not require weakening production catalog validation.

All three native scenarios passed. The 60-frame dialogue scenario verifies:

- No simulation ticks, movement or shots while modal, even with held controls.
- Focus-loss pause and a separate resume press before dialogue advancement.
- French speaker/text refresh preserving visit, generation and selected reply.
- Rest closing without spawning; readiness starting exactly one wave after its final line.
- No simulation on the completion update, and Escape discarding pending readiness.

The dialogue fixture clears enemies through the combat API to reach the between-wave
boundary. It does not establish that a player cleared a wave through shooting; the
retained 350-frame gameplay/audio scenario covers combat, restart and cleanup.
The retained 50-frame shader scenario covers real GPU draws, scope restoration,
failure/retry and disposal; its injected error messages are expected.

Frame 10 of the dialogue replay was visually inspected at 640×480 and retained as
`docs/assets/tutorials/arena-game/06-dialogue.png`. French replies, speaker and
control hints fit, with a visible selection marker. Unsupported arrow glyphs were
replaced with translated words and the panel made opaque before the final capture.
Adjacent `06-provenance.json` records the pre-commit dirty source base separately
from compiler identity. Generated Lua and full capture sequences are not committed.
Existing Kenney and audio assets are carried forward unchanged.

## Limits

This chapter demonstrates modal dialogue with English/French Latin text and a custom
renderer. It does not qualify arbitrary scripts, shaping, bidirectional layout,
unlimited translation length, portraits, voice playback or timed dialogue events.
The next-wave callback is a host-owned effect, not a durable transaction; persistence
belongs to chapter 7. Locale changes refresh presentation without restarting flow.

No installed-extension, Windows/Linux, controller or human usability check was
performed. Native audio tests inspect Source behavior and clocks; videos are silent
and no listening-quality claim is made. Fixed-step replays are functional checks,
not CPU, rendered-FPS, allocation or latency measurements.
