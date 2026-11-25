# Arena game, chapter 3 qualification

Checkpoint commit: `0246e3f5`. Earlier checkpoints remain unchanged. This chapter adds
behavior runners, moving-target collision, player damage and game over; no compiler
or runtime implementation changes were made.

## Provenance and commands

Compiler: development release 0.11.0, embedded revision
`e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin;
SHA-256 `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
This pins the tested binary, not every 0.11.0 distribution.

Headless: Lua 5.4.8. Native: LÖVE 11.5.0, LuaJIT 2.1.1700008891, JIT disabled
by the runner. Final capture `/tmp/tua-arena-c3-qualified`: 350 frames at fixed
1/30-second updates, seed 71, plus direct-update setup/verification trials.
The permanent `docs/assets/tutorials/arena-game/03-provenance.json` records the
pre-commit source base and dirty worktree separately from compiler identity.

From the checkpoint directory:

```sh
../../../../../target/release/tua fmt
../../../../../target/release/tua fmt tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/replay.lua
../../../../../target/release/tua fmt --check
../../../../../target/release/tua fmt --check tests/movement.lua tests/session.lua tests/combat.lua tests/decisions.lua tests/replay.lua
../../../../../target/release/tua check --deny-warnings
../../../../../target/release/tua build
cd build
lua ../tests/movement.lua
lua ../tests/session.lua
lua ../tests/combat.lua
lua ../tests/decisions.lua
```

From repository root, with a fresh output directory:

```sh
python3 scripts/love_replay.py \
  --project examples/tutorials/arena-game/checkpoints/03-enemy-decisions \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/tutorials/arena-game/checkpoints/03-enemy-decisions/tests/replay.lua \
  --frames 350 --fps 30 --seed 71 --output /tmp/tua-arena-c3-qualified --video
make docs-build
make json-check
make diff-check
```

## Results

Formatting, warning-free typed checking, emitted build and all four Lua suites passed.
Retained chapter 2 stationary-collision fixtures explicitly cancel/remove brains so
those fixtures still test their intended geometry. Active-brain tests cover patrol,
chase, attack, 119-tick cooldown exclusion and the next allowed attack, reactive
interruption when leaving range, independent contexts, live position replacement,
removed/reset actors no longer receiving updates, relative moving-target intersection,
lethal projectile priority over a same-tick attack, player death, frozen game over,
and clean restart. Earlier reset, capacity, expiry and cadence checks remain present.

Native replay passed feedback on/off/reduced-motion outcome parity with moving enemies,
aimed hits, pause/focus, reset, natural enemy approach/attack until death, frozen game
over and restart with full health/fresh entities. Captures were inspected at 960×640
and 640×480, including active combat and the final game-over view. A shared walk
animation supplements the existing idle presentation. Gameplay allows enemies to
overlap; this is explicitly not an avoidance/pathfinding example.

The build includes `tuabehavior` only because the source references BehaviorTree.
Generated Lua and replay frame sequences are not committed. Kenney assets remain
unchanged and locally bundled with their license/provenance.

## Selected-compiler typing probes

In a disposable copy, add `src/probe.tua`:

```tua
local Brain = require("brain")
local brain = Brain.create(0)
brain.context.distance = "wrong"
```

`tua check --deny-warnings` rejects this cross-module assignment with TL1003:
expected number, found string. Replacing the last line with the following is
accepted by this selected compiler:

```tua
brain.context.missingField = 1
```

The chapter uses explicitly typed contexts and runner records without `any`, but
this probe does not establish rejection of unknown field names. Recheck this minimal
case against a freshly rebuilt current compiler and its intended record-extension
rules before describing it as a new upstream defect. A future semantic regression
should distinguish permitted table extension from invalid context-field reads/writes
and cover module export/import. No claim of new LSP or installed-extension qualification
is made here.

## Limits

Fixed ticks and a 250 ms catch-up cap remain host policy. Relative collision assumes
linear motion within each tick and circular bodies. The host owns structural changes;
removing/replacing a Brain or required component through another path must cancel its
old runner first. Cancelling a runner does not permanently disable its public update
method; ECS ownership prevents future scheduling after removal.

Eight enemies, 300-pixel detection, 32-pixel attack range, ten player health and one
second per-enemy cooldown are teaching choices. No invulnerability, armor, avoidance,
pathfinding, audio, save feature or performance improvement is claimed. No native
Windows/Linux, controller or human usability testing was performed.
