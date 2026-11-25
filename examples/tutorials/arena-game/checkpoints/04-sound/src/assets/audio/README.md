# Original example audio

These six mono, 22050 Hz, 16-bit PCM WAV files are original procedural tones with
no third-party samples. They are dedicated to the public domain under CC0-1.0:
https://creativecommons.org/publicdomain/zero/1.0/

`shot`, `hit`, `hurt` and `defeat` are short, enveloped sine tones. `harbor` and
`combat` are four-second repeating note patterns, intended as small teaching
fixtures rather than finished music. All waveforms are generated offline by
`../../../scripts/generate_audio.py` (relative to this folder). Run it from any
working directory; Python's standard library is sufficient. WAVs are bundled, so
running the game needs no generator, download or audio-analysis pipeline.

`provenance.json` records format, generator and exact file hashes. Floating-point
math across platforms may change a generated sample; commit any deliberate asset
regeneration with its updated provenance.
