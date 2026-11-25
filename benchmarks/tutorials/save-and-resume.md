# Save/resume qualification

Example commit: `46a9df23`.

Scope: M3, `examples/love-save`. No runtime or compiler implementation changes.
Existing identity `tua-save-example`, slot `profile` and valid v2 records remain
compatible. Historical v1 input now requires its declared schema rather than
being permissively defaulted inside an `any` migration.

## Provenance and commands

- Compiler: release 0.11.0, embedded revision `e0033fc65d3b78f9be8aba3e427aa9a239782354`,
  aarch64-apple-darwin; binary SHA-256
  `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
- LÖVE 11.5.0, LuaJIT 2.1.1700008891; JIT disabled in both harnesses.
- Persistence identity: a fresh `tua-save-tutorial-<UUID>`, shared by two separate
  OS processes and removed after success. The normal example identity was untouched.
- Persistence logs: `/tmp/tua-save-persistence-qualified`; recorded metadata is
  `benchmarks/tutorials/save-persistence-provenance.json`.
- UI replay: 130 frames at fixed 1/30-second updates, seed 71, separate replay
  identity; `/tmp/tua-save-tutorial-qualified`. Selected frames 40 and 65 were inspected.
- Permanent screenshot metadata: `docs/assets/tutorials/save-and-resume/provenance.json`.

From the example directory:

```sh
python3 tests/verify.py --tua ../../target/release/tua \
  --love /Applications/love.app/Contents/MacOS/love \
  --output /tmp/tua-save-persistence-qualified
```

The wrapper passed typed formatting, explicit Lua-test formatting, warning-free
checking and build. It copies the emitted session into a disposable LÖVE host;
only the disposable entry/config files are test harnesses. Production generated
Lua is neither edited nor committed. It verifies the native filesystem/data backend.

From the repository root:

```sh
python3 scripts/love_replay.py --project examples/love-save \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/love-save/tests/replay.lua --frames 130 --fps 30 --seed 71 \
  --output /tmp/tua-save-tutorial-qualified --video
```

Use fresh output directories when repeating these commands.

## Confirmed behavior

- Process one persists score 8; a second process restores it without rewriting on load.
- Missing saves begin at defaults; an unchanged session's update does not create files.
- Dirty progress autosaves after the quiet interval; UI-only notice changes do not write.
- Quit flushes pending healthy progress; disposed bindings are not reused.
- Valid v1 points migrate to v2 score/music preference; load preserves the original bytes
  until a subsequent save. Wrong historical field types produce `invalid_migration_input`.
- Future versions and corrupt envelopes block score changes, autosave, checkpointing and
  quit writes. Reset cancellation leaves both files byte-for-byte unchanged.
- Destructive reset requires a pending confirmation. An injected delete failure keeps
  the prompt and its error; retry succeeds and new defaults can be saved. A failure on
  the second deletion leaves the first copy removed and keeps writes blocked even
  after prompt cancellation.
- With one older valid copy and a damaged newer copy, progress remains blocked until
  explicit recovery acceptance; subsequent writes produce a healthy load.
- An injected write failure keeps the score in memory, surfaces its error and permits
  an explicit successful retry.
- Native UI replay covers points, checkpoint, confirmation input ownership, cancel and
  confirmed reset. Progress and reset screens are plain, without decorative assets.

## Editor/type checks and follow-up

A temporary LSP session confirmed imported session hover and `score`/`musicOn`
completion on `session.snapshot()`. Disposable source edits were rejected for:

- Changing migration input to `{ points: string }` against the number-valued historical schema.
- Returning a string-valued `score` from that migration.

Both report TL1003. These are protocol/checker checks, not installed-extension UI tests.

One non-blocking qualification follow-up remains: in this selected binary, changing
only the migration result to the following was accepted by `tua check`:

```tua
[1] = function(old: { points: number })
  return { score = old.pointz, musicOn = true }
end,
```

Reproduce by copying the example to a disposable directory and replacing `old.points`
with `old.pointz` in `src/profile.tua`. Expected stricter behavior is an unknown-field
or invalid-result diagnostic; actual result was a successful check. Rebuild the current
compiler before classifying this as an outstanding upstream issue: this result identifies
the selected binary, not every newer source revision. Preserve this repro if the gap
remains. Runtime validation still protects historical input and migrated output; the
working example uses the correct field. Do not claim the editor detects every typo.

## Limits

Only native macOS LÖVE was exercised. No power-loss, concurrent writers, cloud-sync,
Windows, Linux or browser storage qualification was performed. Functional completion
is not a storage-latency or durability benchmark. Missing/failed data stays blocked;
this sample is not a transactional inventory service.
