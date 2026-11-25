# Dialogue audio

An optional, copyable **host adapter**, written in Tua. The core dialogue runtime
still needs no audio device. This example plays original short synthetic blips
and a longer synthetic recording fixture through ordinary LÖVE Sources.
It does not contain Animal Crossing assets or recorded human speech.

From this directory:

```sh
tua check
tua build
love build
```

Use a compiler containing schema-10 dialogue support. The extension Play button
also works when this folder is the project root.

| Control | Behavior |
| --- | --- |
| Space | Finish text silently; the next press advances and stops the recording. |
| P | Suspend/resume dialogue, reveal and owned audio sources. |
| M / V | Toggle chatter / recording mute independently. |
| F | Toggle 4× reveal speed; recording speed stays unchanged. |
| L | Switch English/French; restart any recording with the requested fallback. |
| R | Restart the conversation, stopping previous audio. |
| Escape | Cancel the conversation and stop audio. |

The initial French locale intentionally has no recording: the voiced line falls
back to its English source cue. Another line references a missing cue to show
that subtitles and chatter continue. A final line explicitly disables chatter.

## Use your own sounds

- `src/characters.tua` declares typed, shared speech profiles. Each line opts in
  with `@speech character`, overrides it, or uses `@speech off`.
- `src/backend.tua` maps sample-set IDs to preallocated short-sound pools and
  `cue:locale` keys to recordings. Replace `assets/recording.wav` with your own
  licensed recording; keep the matching `@voice` metadata in `scene.tdialogue`.
- `src/adapter.tua` owns reveal timing and per-player audio policy. Construct one
  adapter and one backend per player; never share their mutable Sources.
- `src/main.tua` keeps subtitle drawing and input policy separate from playback.

There are two Sources per chatter sample set and one recording Source: five
Sources per conversation. Overflow is dropped. No Sources are created during
playback. Long frames emit at most one blip and never enqueue catch-up sounds.
A private Park–Miller seed varies pitch without consuming `math.random` or game
RNG. Variation scales around the pitch range midpoint; zero uses that midpoint.
Mute consumes reveal progress, so unmuting cannot replay missed cues.

The sample's `wordStart` callback recognizes ASCII whitespace/punctuation
boundaries. It is deliberately replaceable: languages without those boundaries
need an appropriate segmenter. Unicode reveal still uses Tua's grapheme handling;
combining marks remain attached and punctuation/emoji do not become chatter.
Pronunciation hints are not speech synthesis or phoneme selection in this sample.

A recording suppresses chatter while actually playing unless `mixSpeech` is true.
Muting a recording keeps its playback clock and suppression policy. Finishing
text keeps the recording; advancing, cancellation, restart and disposal stop it.
Locale/presentation changes stop and restart recording selection. Suspension
pauses only owned playing Sources, and resume does not revive muted chatter.
Apply `audio.update(player:snapshot(), 0)` immediately after navigation so old
sounds stop before the next frame. Call `audio.dispose()` when releasing a player.
Chatter gain changes apply to subsequent blips; muting stops existing blips.

Actions are not skipped by this adapter. The full M7 policy for cancellable and
non-cancellable actions remains separate; it never bypasses the player's state.

## Verification and assets

```sh
python3 verify.py --tua ../../../target/debug/tua
python3 verify.py --love /Applications/love.app/Contents/MacOS/love --render
```

The script formats/checks, builds typed source, executes emitted Lua regressions,
and optionally runs native audio plus 100 rendered frames with JIT on and off.
It copies the build to a temporary directory for its test entrypoint; generated
production output is never patched. Native checks need an available audio device.
`--render` exercises controls; it is not a listening-quality assessment.

The three WAV files are original generated fixtures under the repository license.
Regenerate them with `python3 generate_samples.py`. No external audio dependency
or proprietary game recording is included.
