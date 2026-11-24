# M7 optional dialogue audio adapter

Implemented as typed host code in `examples/dialogue-audio`, on the feature branch.
The compiler and core dialogue runtime are unchanged in this slice. No merge or
publication is included.

## Ownership and bounds

Each conversation owns one controller and one LÖVE backend: two Sources for each
of two chatter sample sets, plus one recording Source. Playback reuses these five
Sources, dropping pool overflow. Reveal updates coalesce to at most one blip per
update and discard missed cues; they never enqueue catch-up audio. A private
Park–Miller generator varies pitch without touching gameplay RNG.

Recording locale fallback, suppression while playing, explicit mixing, separate
mute controls, pause/resume and lifecycle cleanup are demonstrated. Finish reveals
text silently while allowing a recording to continue. Advance, cancellation,
restart and disposal stop old audio. Presentation/locale refresh restarts recording
selection. Muted chatter cannot resume from a previously paused source list.

The word-boundary callback is intentionally ASCII whitespace/punctuation based;
replace it for other language segmentation. Grapheme reveal remains Unicode-aware.
Pronunciation hints do not synthesize speech. All supplied WAV files are original
synthetic fixtures, including the longer stand-in for a licensed voice recording.
The example does not demonstrate actual human voice acting or phoneme mapping.
The full policy for skipping pending host actions remains open in M7.

## Verification

- Shared formatter and explicit Lua-test formatting checks pass.
- `tua check` and `tua build` pass for all typed modules and the authored dialogue.
- Emitted controller/backend regressions pass under Lua 5.4.8 and LÖVE's LuaJIT.
- Fake backend tests cover cue rate limits, punctuation/emoji, combining text,
  long frames, silent finish, locale refresh, voice fallback/mixing, mute, repeat
  updates, disposal, seed reproducibility and gameplay RNG isolation.
- Source backend tests cover bounded pool exhaustion, reuse, independent owners,
  paused mute/resume and complete release.
- Native LÖVE 11.5 / LuaJIT 2.1.1700008891 checks use real audio Sources with JIT
  explicitly on and off. Both modes pass pause/resume, concurrent ownership,
  release and zero active Sources after cleanup.
- Each native run also renders 100 frames and exercises pause, advance, mute,
  locale change and restart. Both report `RENDERED_AUDIO_OK`.
- Documentation build, JSON metadata and whitespace checks pass. The full
  repository gate was not repeated for this example/docs-only slice; no core,
  compiler, language-server or extension implementation changed.

These are functional playback and rendered smoke checks, not a listening-quality,
frame-rate, allocation or installed-editor UI assessment. No standalone Lua 5.1
or LuaJIT executable was available; LÖVE supplies LuaJIT compatibility coverage.

## Reproduction and provenance

Run `python3 examples/dialogue-audio/verify.py --love /Applications/love.app/Contents/MacOS/love --render`
from the repository root. The script uses an isolated copy for its test entrypoint;
it does not patch generated production files. Source tests and original sample
generation are tracked beside the example.

Final qualification uses Tua 0.11.0, debug,
`aarch64-apple-darwin`, revision `0d2e6b5d7ba7af723fb54418d154ce2bf127f5a9` plus
example/documentation working changes. Compiler SHA-256:
`9bdf86c07e8f6d53df84c28133be50ecc31777b0cf7885da01fa72dc6f5906ba`.
