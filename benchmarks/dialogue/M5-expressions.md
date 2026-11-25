# M5 character expressions

Baseline: `330e1f33610ed0407f31c5f83a033c2bfc46d626`, with this milestone's local
changes. No release/version change or publication is included.

## Implemented contract

Named character `expressions` contain a required nonempty `portrait` asset ID and
optional nonempty `animation` name. `@expression name` follows an imported-character
utterance, before choices. It applies only to that line. Omission uses the character's
default portrait, or nil; no expression state persists between lines or speakers.

Schema 7 carries the selection and registry into construction, where selected strings
are validated and copied. Snapshots expose `expression`, resolved `portrait`, and
`portraitAnimation`. Registries and GPU/playback state are not exposed in snapshots.
The host owns loading, unknown-asset handling, animation selection and resource lifetime.

Shared analysis validates selected names/metadata and provides completion, hover and
character-declaration navigation through imports/re-exports and unsaved edits. It
never executes project code or loads portrait images. A widened dictionary does not
provide a declared expression-name set. Host animation IDs are metadata, not a claim
that the dialogue compiler can inspect arbitrary sprite registries.

## Verification and provenance

Compiler version 0.11.0, dirty base revision above, debug aarch64-apple-darwin.
Executable SHA-256:
`06b40ffbd1085e26d8de69cddba4d863dac04f797651a870b92accaa50c7e4ac`.

Focused dialogue tests passed: 30 core cases plus action/event/context/condition,
asset and expression integrations. New cases cover independent players, two speakers,
portrait omission/default/reset, snapshot and source-mutation isolation, locale change,
cancel/restart/dispose, malformed metadata, unknown expressions, invalid placement,
re-export navigation, completion and diagnostic refresh. Dotted local speaker names
are rejected for expression selection; they cannot reach an imported-character emitter
path. Schema 7 composes with actions, typed context and both event clocks. Existing
portrait-only fixtures retain their earlier schemas; unsupported schema 8 is rejected.

The example checks, builds, formats and runs under Lua 5.4.8. Its emitted modules and
the maintained lifecycle oracle also pass in LÖVE 11.5.0 / LuaJIT 2.1.1700008891, with
`jit.status()` confirmed true and false in separate runs. The expanded oracle uses
Save for its composition case: the isolated native harness explicitly references
Save in typed source so the rebuilt compiler emits that dependency. An initial
harness omitted it; that harness setup was corrected before the final native passes.

The optional sprite recipe was checked/built in an isolated Tua project using the
repository's existing astronaut atlas and the documented 32px/four-frame configuration.
Native LÖVE verifies two distinct sprite players and animation objects, different
positions after independent update durations, and successful draw calls. This is a
functional adapter smoke check, not a visual-design evaluation or performance claim.
No new image artwork or game changes are included.

Standalone Lua 5.1 and standalone LuaJIT are unavailable; LÖVE supplies the LuaJIT
coverage. No installed VS Code UI test was performed. Shared semantic/editor and
TextMate checks cover the authoring behavior without claiming visible UI latency.

## Reproduce

```sh
cargo build -p tua-cli
RUST_MIN_STACK=16777216 cargo test -p tua-core dialogue
make verify
make docs-build
(cd examples/dialogue/expressions && ../../target/debug/tua fmt --check && ../../target/debug/tua check && ../../target/debug/tua build)
(cd examples/dialogue/expressions/build && lua main.lua)
```

The maintained oracle is `crates/tua-core/tests/fixtures/dialogue_expressions.lua`.
For native checks, copy the emitted example and oracle to an isolated LÖVE host,
with the typed Save dependency described above. Toggle JIT explicitly. For the
sprite recipe, construct two players from one compiled sheet; update them by 0.2
and 0.05 seconds at 8 FPS and verify separate state before drawing both.

## Final gates

`make verify` passed: 2,128 core tests (one ignored), 19 core integration tests,
508 language-server tests, CLI/build-cache coverage, Clippy with warnings denied,
version/schema/model checks, reference and mutant evaluations, feature examples,
292 extension tests and grammar/theme/activation checks, JSON and whitespace.
The minimal dotted-local-speaker regression was added during that run and the
expression integration target was rerun successfully afterward. Production code
was unchanged during verification. Final example formatting/check/build passed;
explicit contract/runtime/oracle formatting passed separately from Rust formatting.
Strict docs builds passed. The static-image recipe was also type-checked in an
isolated LÖVE-target project. Native lifecycle and sprite smoke checks were rerun
using the final compiler hash recorded above. No push, publish or installation.
