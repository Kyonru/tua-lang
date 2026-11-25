# Game flow qualification

Example commit: `257701a2`.

Scope: M2, extending `examples/pause-input`; no compiler/runtime implementation
changes. The previous regression scenario now explicitly enters playing in setup;
normal launch starts at the title. Its paused movement, live UI, held-input,
practice gate and simultaneous pause/fire assertions remain.

## Provenance

- Compiler release 0.11.0, embedded revision `e0033fc65d3b78f9be8aba3e427aa9a239782354`,
  aarch64-apple-darwin; binary SHA-256
  `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
- Native LÖVE 11.5.0 / LuaJIT 2.1.1700008891; JIT disabled by replay runner.
- Seed 71, fixed 1/30-second updates. Existing scenario: 60 frames; new flow: 130.
- Final flow capture: `/tmp/tua-game-flow-final`; legacy regression:
  `/tmp/tua-game-flow-legacy-final`. These temporary paths are local evidence.
- Permanent screenshot and capture metadata: `docs/assets/tutorials/game-flow/`.
- No audio or external assets; these are functional checks, not timing benchmarks.

## Verification

Inside the example:

```sh
../../target/release/tua fmt --check
../../target/release/tua check --deny-warnings
../../target/release/tua build
```

All passed. Both scenarios are typed source and covered by the formatter/checker.
The README contains the native replay commands. Both final replays passed.

The new scenario checks:

- Title does not advance gameplay; start consumes its input frame.
- Holding Space across start or resume does not fire.
- Pausing consumes simultaneous fire; gameplay time and movement stay frozen.
- UI caption fade finishes while paused; explicit resume does not catch up time.
- Focus loss pauses; unfocused confirmation is ignored; refocus stays paused.
- Restart from pause consumes simultaneous fire and resets counters.
- Returning to title freezes gameplay; starting again resets it.
- 100 repeated pause/resume/reset cycles produce identical first-step movement
  without stale tween updates or shot accumulation.

Inspected title and pause screenshots from frames 10 and 55. Layout is plain text
and a moving marker with no grid, decorative cards or unnecessary assets.

A temporary LSP JSON-RPC session confirmed state-machine hover and typed
`pause`/`resume`/`restart` completion. A disposable project changed a state check to
`flow:is("missing")`; the compiler rejected it with TL1003. These were protocol and
checker checks, not installed-extension UI or static-graph visual qualification.

The initial lifecycle probe assumed a linear tween. The existing tween uses its
runtime's default easing; the final test compares each reset's first step with a
known fresh run instead of imposing an unrelated motion change.

## Limits

Only native macOS LÖVE was qualified. No browser or Windows/Linux runtime was run.
The sample simulates shots with a counter and movement with a tween; it does not
claim to pause audio, external work or another game's systems automatically.
