# Chapter 6: conversation and language

Extends the same game with English/French UI catalogs and a modal between-wave
conversation authored in its own `.tdialogue` file, using a shared character module.

## Run

```sh
tua check --deny-warnings
tua build
love build
```

## Controls

Play with the previous controls. **Tab** switches language at any time. After clearing
a wave and waiting for bullets to expire, **N** talks to the harbor keeper. In dialogue,
**Enter** advances/chooses, **Up/Down** selects a reply, and **Escape** closes. R/Backspace
cancel the conversation and retain their restart/title behavior. Audio/settings controls
remain available. Movement and firing never receive dialogue input.

## Expected behavior

Choosing readiness and completing its final line starts one next wave. Choosing rest
or closing does not. Locale switching refreshes text and speaker name without starting
a new visit, moving selection or replaying the next-wave request. Focus loss suspends
input; after return, the first Enter resumes, and a later Enter advances dialogue.

The UI uses complete English/French catalogs. Tests temporarily omit one French entry
to check English fallback; the shipped catalog stays warning-free. The default LÖVE
font was checked for these Latin-script strings, not arbitrary scripts. There are no
portraits, typewriter reveal, voice recordings, timed events or persistent locale yet.

## Try a change

Edit the prose in `src/interlude.tdialogue` and matching catalog values, preserving the
stable IDs. Add a longer translated reply and inspect it at 640×480. Change `cast.tua`
to share the same named character with another conversation. Keep gameplay requests
out of the translation resolver.

## Verification

```sh
tua fmt --check
tua fmt --check src/locales/en.lua src/locales/fr.lua tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/soundscape.lua tests/replay.lua tests/shader-replay.lua tests/conversation.lua tests/dialogue-replay.lua
tua check --deny-warnings
tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
lua ../tests/decisions.lua
lua ../tests/soundscape.lua
lua ../tests/conversation.lua
```

From repository root, use a fresh output directory:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/06-conversation-and-language \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/06-conversation-and-language/tests/dialogue-replay.lua \
  --frames 60 --fps 30 --seed 71 --output /tmp/arena-dialogue-replay --capture
```

Run the inherited `tests/replay.lua` with 350 frames and `tests/shader-replay.lua` with
50 frames too. The dialogue scenario clears enemies through lifecycle APIs to reach
the interlude; inherited gameplay replay covers real combat. These are agent-led
functional scenarios, not human usability or international-font qualification.

## Next steps

[Read the chapter](../../../../../docs/tutorials/arena-game/conversation-and-language.md).
Continue with [chapter 7: save progress](../07-save-progress/README.md).
