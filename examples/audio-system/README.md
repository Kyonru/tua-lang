# Audio System Example

This self-contained LOVE project demonstrates Tua's compiler-aware audio
workflow:

- `audioSystem({...})` derives typed bus, cue, effect, and music-scene names;
- cue playback uses priority and per-cue voice limits;
- music scenes consume generated tempo and beat-grid metadata;
- beat and bar events use the same typed signal API as other Tua events;
- the built-in `audio-analysis` asset pipeline caches waveform, envelope, and
  tempo analysis under `.tua/cache` and materializes a typed catalog under
  `src/generated/audio`;
- `tua build` emits `tuaaudio.lua` and its `tuasignal.lua` and `tuastore.lua`
  runtime dependencies only because this project uses the audio helper.

Build or run it from the repository root:

```bash
make build-audio-system
make run-audio-system
```

The included WAV files are small synthetic fixtures. `sample2.wav` has an
intentionally ambiguous detected tempo, so the analyzer may emit `TL2012`.
The scene authors its intended 90 BPM timing explicitly; the warning documents
why automatic quantized timing should not rely on that estimate.

Generated Lua is written to the ignored `build/` directory. The generated
catalog sources are tracked so type checking and the VS Code Audio Explorer
work immediately after checkout; rerunning the build refreshes them when the
audio inputs or pipeline options change.
