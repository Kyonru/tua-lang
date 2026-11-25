# Chapter 4: sound

The same game now has shot/hit/hurt/defeat cues, title/combat music scenes, independent
bus volume controls and mute. Enemy shadows carry forward from chapter 3.

## Run

```sh
tua check --deny-warnings
tua build
love build
```

## Controls

The prior movement/combat/menu controls remain. **1/2** lower/raise music volume,
**3/4** lower/raise effects volume, and **0** toggles master mute. F/M remain visual
feedback/reduced-motion controls; they do not change audio or gameplay.

## Expected behavior

Title and game over use the harbor loop; gameplay uses the combat loop. Pause stops
short effects and pauses music; resume continues its transport. Losing focus also
pauses music. Reset/scene changes stop old effects and replace the music voice with
a short fade-in. This is not a crossfade. Volume/mute preferences survive restart,
but are not persisted across application launches yet.

The host summarizes each cue category once per outer update, rather than replaying
all accumulated simulation ticks as a burst. Damage and firing cadence are unchanged.
Concurrent effects are bounded by cue and bus limits. Sources are created from the
bundled original WAV fixtures; Kenney art remains unchanged.

## Try a change

Adjust default gains in `soundscape.tua`, a cue's voice limit in `audio-backend.tua`,
or replace a WAV with a licensed recording. Keep sample creation out of `update`.
Try mute while firing: gameplay continues. Pause/resume should not restart music.

## Verification

```sh
tua fmt --check
tua fmt --check tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/soundscape.lua tests/replay.lua
tua check --deny-warnings
tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
lua ../tests/decisions.lua
lua ../tests/soundscape.lua
```

From repository root, using a fresh output folder:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/04-sound \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/04-sound/tests/replay.lua \
  --frames 350 --fps 30 --seed 71 --output /tmp/arena-sound-replay --video
```

The replay verifies real LÖVE playback, a stopped/resumed Source clock, bus settings,
30 restarts without extra music voices and zero active Sources after disposal. It
waits 80 ms twice to sample the real playback clock; those are not timing benchmarks.
Its MP4 is silent: the runner does not capture audio. Run the game for listening.
The fake-backend suite separately tests policy deterministically.

## Next steps

[Read the chapter](../../../../../docs/tutorials/arena-game/sound.md).
Next: a scoped hit shader and shader loading/failure ownership in the same game.
