# M6 reveal foundation

This is the first M6 implementation slice, based on Tua revision
`3cea755da116356b8007702119c5fff8be145d0b`. M6 remains open: styled spans, authored
pause/speed/pronunciation syntax, and authored-to-localized style mappings are not
implemented here. Audio profiles/adapters remain M7.

## Contracts and evidence

`Dialogue.reveal` consumes the presentation fields of an owned dialogue snapshot.
It does not require particular notification payload types; imported typed-event
snapshots are covered by a regression. Controllers reveal final resolved text and
never interpret substituted or translated strings as commands.

The private segmenter implements Unicode 15.1 extended grapheme clusters from
UAX #29 revision 43, including Indic conjunct, pictographic ZWJ and RI pairing rules.
All 1,187 official GraphemeBreakTest cases pass. Tables are generated from pinned,
SHA-256-verified official data; `scripts/unicode-15.1-manifest.json` records origins.
`tuadialogue/unicode.LICENSE` ships with the runtime data. This is a pinned version,
not a claim of conformance to future Unicode releases.

A cluster exposes inclusive one-based UTF-8 byte ranges, its text/index, and whether
it contains a Unicode letter/number. Invalid UTF-8 and text over 65,536 bytes fail
before replacing an active presentation. These are logical clusters, not shaped
glyphs, visual positions, phonemes, or word boundaries. Fonts, shaping, bidi layout,
line breaking, audio and resource ownership remain host responsibilities.

Speech is opt-in. Each update emits zero or one eligible newly revealed cluster,
coalescing a long frame to the last eligible cluster. Interval-suppressed cues are
dropped with no retained queue. Controller instance/sequence plus dialogue identity
let hosts reject stale playback. Finish/clear invalidate prior tokens; completed
locale/context refresh and reduced motion reveal silently. Two controllers do not
share progress. The controller never advances the dialogue or consumes gameplay RNG.

Tests cover UTF-8 rejection, byte mappings, combining/emoji/flag/Indic/CJK examples,
owned snapshots/clusters, silent punctuation/whitespace, independent players,
finish/cancel/restart/dispose, locale refresh, placeholder text, cue coalescing and
intervals, finite-input validation, and equivalent frame partitions. A saturation
regression covers rate 7 with 61 clusters: a huge dt must finish despite floating-point
division/multiplication rounding. A named consumed-field input contract prevents
unrelated typed notification payloads from making valid reveal calls fail checking.

## Reproduce

```sh
cargo build -p tua-cli
RUST_MIN_STACK=16777216 cargo test -p tua-core dialogue_reveal
make verify
make docs-build
(cd examples/dialogue/reveal && ../../target/debug/tua fmt --check && ../../target/debug/tua check && ../../target/debug/tua build)
(cd examples/dialogue/reveal/build && lua main.lua)
```

For Unicode regeneration, fetch the files named in `scripts/unicode-15.1-manifest.json`
into an isolated directory, then run:

```sh
python3 scripts/generate-dialogue-unicode.py /path/to/unicode-data --check
```

Omit `--check` to regenerate the table. The license URL is a snapshot rather than a
versioned URL; the pinned license bytes are also available in
`vendor/runtime/tuadialogue/unicode.LICENSE`. Copy those bytes as `LICENSE.txt` when
reproducing against the manifest if the upstream license has since changed.
Generated table output uses UTF-8/LF bytes independently of host newline settings.
The checked-in corpus is `crates/tua-core/tests/fixtures/unicode/GraphemeBreakTest-15.1.0.txt`.

Native verification copies the compiler-emitted example modules to an isolated,
windowless LÖVE host, runs `dialogue_reveal.lua`, then calls the function exported by
`dialogue_graphemes.lua` with the official corpus text. Run separately after
`jit.on()` and `jit.off()`, recording `jit.status()`. There is no rendering or audio
playback in this check and no visual, timing or multilingual-shaping claim.

## Integrated verification

Verified on macOS/aarch64 with the final rebuilt Tua 0.11.0 debug compiler, based on
`3cea755da116356b8007702119c5fff8be145d0b` plus this change (dirty build).
Compiler executable SHA-256:
`543cf9228018618f8677a60d6e371b3d870815aa4ba85e2fc3ca592be644ab46`.

- `make verify` passed: 2,129 core unit tests (one ignored), 508 language-server
  tests, CLI/build-cache and integration tests, workspace Clippy, model contract
  evaluations, and 292 extension tests. After the final consumed-field contract
  correction, the complete core suite and core Clippy passed again.
- Strict documentation build, generated Unicode checksums/table comparison,
  Rust formatting, explicit Tua/Lua formatting checks, and diff checks passed.
- The final compiler checked the cross-module typed-notification reproduction,
  checked/built the example, and its emitted example passed under Lua 5.4.8.
- LÖVE 11.5.0 / LuaJIT 2.1.1700008891 passed the emitted example, lifecycle oracle,
  and all 1,187 official Unicode cases separately with JIT enabled and disabled.

Standalone Lua 5.1 and LuaJIT executables were unavailable. Native checks were
headless; audio playback, rendered typography and installed-editor interaction
were not tested. These are correctness results, not performance measurements.
