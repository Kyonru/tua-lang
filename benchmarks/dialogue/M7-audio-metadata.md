# M7 voice metadata and shared speech profiles

Implemented on `feature/tdialogue-rich-text`; not merged or published.
This slice describes host audio. It does not load samples, play sound, configure
reveal cadence, or own audio sources. The optional LÖVE adapter and its audible
lifecycle qualification remain pending in M7.

## Contract

`Dialogue.speechProfile` validates and copies a plain profile with a sample-set
identifier, bounded pitch range, variation, gain, minimum cue interval and
character/word cadence. Imported character modules retain typed completion and
hover. A line explicitly selects `@speech character`, an inline JSON profile,
or `@speech off`; omission never inherits the preceding line's profile.

`@voice` records a cue identifier, source locale, explicit `silent`/`source`
fallback policy, and optional mixing flag. Each presentation adds the requested
locale, independently of subtitle resolution. The host resolves availability and
keeps subtitles visible when audio is missing. Chatter suppression while a
recording actually plays is host policy, not a side effect of declaring a cue.

Media directives select schema 10. Earlier documents keep their existing schema
selection and behavior. Schema 10 composes prior rich text, waits, events,
actions and typed context. Replacement runtimes need schema-10 support for media
documents. Profiles and voice views are copied at player/presentation boundaries;
mutating a source profile or one snapshot cannot alter another player.

## Compatibility finding

Integration verification exposed a shared parser defect: normalizing an optional
union field (`speech?: Profile | "off"`) placed the optional marker on the final
union member, then lost the field's optional flag. The parser now recovers that
flag while retaining all alternatives. Parser and checker regressions cover
omission, nil, valid alternatives, rejected values and nullable reads. The existing
legacy inline `Dialogue.new` CLI regression passes unchanged.

## Qualification

- Parser tests cover bounded values, explicit fallback, duplicate/unknown fields,
  duplicate directives, imported character requirements and invalid placement.
- Imported typing tests cover the constructor, forwarded modules, completion,
  hover, invalid profile values and unsaved profile changes.
- Runtime tests cover validation, ownership, concurrent players, locale refresh
  during suspension, explicit silence and no metadata leakage between lines.
- `examples/dialogue-expressions` passes format/check/build without warnings and
  its emitted Lua executes successfully.
- A headless native harness runs that emitted example plus expression and media
  oracles under LÖVE 11.5 / LuaJIT 2.1.1700008891, with JIT explicitly on and off.
  Both pass. The expression oracle's persistence checks use the current bundled
  Save helpers in the temporary harness; these are not added to the example.
- Graphics and audio are disabled in this harness. No audible playback, installed
  editor UI, source reuse, polyphony or speech cadence processing is claimed.

## Provenance

Tua 0.11.0 debug, `aarch64-apple-darwin`, revision
`84b3e98f715d4591dfd6a184bc3077b8b94bdfac` plus this slice's working changes.
Compiler SHA-256:
`5a408a8a801987d7f2bf2e31a5315d33dbacf10604bbceed1961a49862466e2b`.
Standalone runtime qualification uses Lua 5.4.8. No standalone Lua 5.1 or LuaJIT
executable was available; native LÖVE supplies the LuaJIT compatibility checks.

## Repository gate

`make verify` passed: workspace tests (including 2,156 core tests, 509 LSP
tests and all 167 CLI integration tests), Clippy, model-language reference and
negative controls, feature examples, 292 extension tests, grammar and metadata
checks. Existing ignored tests remain ignored. `make docs-build`, model-contract
drift, example format/check/build, explicit runtime/fixture formatting and
`git diff --check` also passed. No installed VS Code UI check was performed.
