# Enemy-wave qualification

Example commit: `936b615a`. Scope: tutorial source and documentation only; no
compiler/runtime implementation changes. Kenney Platformer Characters assets are
CC0 and unchanged; their archive and file hashes are recorded beside the asset.

## Provenance

- Compiler: development 0.11.0 release binary, embedded revision
  `e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin.
- Compiler SHA-256: `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
  This is not a claim that every installed 0.11.0 binary has the same behavior.
- Headless lifecycle suite: Lua 5.4.8 on emitted modules.
- Rendered replay: LÖVE 11.5.0, LuaJIT 2.1.1700008891, JIT disabled by runner;
  150 frames at fixed 1/30-second updates, seed 71, fresh isolated save identity.
- Replay output: `/tmp/tua-enemy-wave-qualified`; frame 31 visually inspected and
  retained as `docs/assets/tutorials/enemy-wave/defeat.png`.
- Full replay metadata is in that directory's `provenance.json`. Its source
  revision is the pre-change base with a dirty worktree, not the final example
  commit. Compiler identity is recorded independently.

## Checks

From `examples/tutorials/enemy-wave`:

```sh
../../../target/release/tua fmt
../../../target/release/tua fmt tests/lifecycle.lua tests/replay.lua
../../../target/release/tua fmt --check
../../../target/release/tua fmt --check tests/lifecycle.lua tests/replay.lua
../../../target/release/tua check --deny-warnings
../../../target/release/tua build
cd build
lua ../tests/lifecycle.lua
```

All passed. Lifecycle assertions cover nonlethal damage, single defeat accounting,
removal of all components, no movement after destruction, 100 resets, replaced
and removed health payloads, deterministic target ties, explicit next-wave limits,
ring expiration/reset/reuse, the 16-ring bound and idempotent disposal. Emission
includes `tuaecs`, `tuaobjectpool` and `tuasprite`; no dialogue runtime is emitted.
Generated output is ignored, not authored or committed.

From the repository root:

```sh
python3 scripts/love_replay.py --project examples/tutorials/enemy-wave \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/enemy-wave/tests/replay.lua \
  --frames 150 --fps 30 --seed 71 --output /tmp/tua-enemy-wave-qualified --video
make docs-build
make json-check
make diff-check
```

Native replay passed actual mouse/key callbacks, damage, defeat rings and expiry,
wave clearing, duplicate next-wave input and reset with an active ring. The scene
has no grid background. The capture verifies functional rendering, not FPS,
allocation savings or human usability. Windows/Linux and an installed VS Code
extension were not exercised.

## Editor qualification and limitation

A stdio LSP probe against the selected binary completed `current` and `maximum`
after `health.` in the imported-component consumer. A disposable copy replacing
`current = maximum` with `current = "wrong"` failed checking with TL5002:
`ECS component field current expected number, found string`.

However, hover for `readHealth` displayed
`(entity: EcsEntityId) -> any | nil`, rather than the specific health record.
Completion and write validation passing do not establish complete reader inference
or precise hover. Reproduce against a freshly rebuilt current compiler before
filing a new upstream defect; this qualification used the pinned development
binary above. A focused future regression should assert both the reader's
nullable payload signature and rejection of invalid field access after import.

No runtime speedup is claimed for cosmetic pooling. The shared animation clock,
point-click targeting, eight-enemy policy and fixed window are teaching choices.
