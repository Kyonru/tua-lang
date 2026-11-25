# Arena game, chapter 8 qualification

Checkpoint commit: `1cb2cbcd`. Adds bounded 8/32/128-enemy selection and replaces a
second full enemy query with a reused list of actual attack intents. The frozen
pre-change typed combat module remains in `tests/reference-combat.tua`. No Tua
compiler, runtime or extension changes. Earlier checkpoints are unchanged.

## Provenance and methodology

Host: Apple M3 Max, macOS 26.6.1, arm64. Compiler development release 0.11.0,
embedded revision `e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin;
SHA-256 `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
This pinned binary is distinct from source HEAD and other 0.11.0 distributions.
Native LÖVE 11.5.0, LuaJIT 2.1.1700008891, JIT disabled explicitly. Retained headless
regressions and differential comparison ran under Lua 5.4.8.

`arena-game-08-results.json` retains both valid batches, all per-update timing samples,
trial medians/p95s, outcome hashes, source/emitted hashes and runtime/binary identities.
Each batch uses five independent trials per variant/size, alternating baseline-first
and candidate-first order. Seed 71; 120 warm-up updates, 600 measured updates; 1/60 s
per call, two simulation ticks per call, one-second reset episodes. Every timed update
asserts a living player and exactly two advanced ticks. Timers surround only
`combat.update`, with GC running at default runtime settings; reset, output hashing,
draw and sleep are outside that block. GC from reset garbage can affect later updates.

Separate allocation processes warm up, reset, collect, stop GC and execute 60 updates
without another reset. The pre/post heap difference is allocated KiB for that block.
After restarting and collecting GC, a second reading gives retained-heap delta from
the pre-block baseline. One probe per variant/size in each batch is retained.

An initial two-second fixture allowed death at 128 enemies. Its idle measurements
were rejected before the valid batches; they are not included in reported numbers.
The active-update assertions prevent silently reintroducing that error. No absolute
performance threshold is encoded in tests.

## Timing results

All values below are milliseconds. Each median is the median of five trial medians;
each p95 is the median of the five trial p95s. Ranges are min/max trial medians.
Columns show **reference / candidate**, with no filtering of unfavorable trials.

| Batch | Enemies | Median | p95 | Trial median range |
| --- | ---: | --- | --- | --- |
| 1 | 8 | 0.0473 / 0.0463 | 0.0604 / 0.1003 | 0.0468–0.0497 / 0.0458–0.0514 |
| 1 | 32 | 0.1361 / 0.1322 | 0.1583 / 0.1530 | 0.1336–0.1385 / 0.1311–0.1346 |
| 1 | 128 | 0.5152 / 0.5087 | 0.5433 / 0.5485 | 0.5129–0.5192 / 0.4979–0.5130 |
| 2 | 8 | 0.0541 / 0.0719 | 0.0762 / 0.1020 | 0.0467–0.0745 / 0.0464–0.0743 |
| 2 | 32 | 0.1845 / 0.1866 | 0.2261 / 0.2245 | 0.1772–0.1894 / 0.1560–0.1871 |
| 2 | 128 | 0.5118 / 0.5074 | 0.5489 / 0.5421 | 0.5079–0.5249 / 0.4947–0.5226 |

There is no reliable overall CPU/p95 win. Candidate medians were slightly lower at
128 enemies in both batches, but ranges overlap; smaller workloads varied substantially
and batch 2 includes worse candidate medians. These uninstrumented simulation-wall
measurements do not establish rendered FPS, GPU cost, a universal enemy ceiling or
an improvement to the complete game's frame time.

## Allocation and retained memory

Both valid batches produced the same allocated-byte results:

| Enemies | Reference allocated KiB | Candidate allocated KiB | Reduction |
| --- | ---: | ---: | ---: |
| 8 | 195.961 | 191.273 | 4.688 KiB |
| 32 | 116.375 | 111.688 | 4.688 KiB |
| 128 | 188.578 | 183.891 | 4.688 KiB |

The removed full query also removes its per-tick callback allocation. The 4.6875 KiB
reduction over 120 ticks corresponds to 40 bytes per tick on this LuaJIT build.
Allocation is not monotonic with enemy count because the workload changes contact,
projectile and behavior activity, not only the number of identical static records.

Post-collection retained deltas were 8.914 KiB (8 enemies), 34.813 KiB (32), and about
150.477 KiB (128) for both variants; one baseline 128 probe was 150.977 KiB. These are
retained deltas for the probe, not total game memory. The approaching-enemy fixture
usually has no attack intents. A dense attacking fixture can grow the reused ID array;
correctness covers that path, but these figures do not quantify its peak capacity.

The candidate is retained for the consistent allocation reduction and removal of an
unnecessary full pass. It is not described as solving the dominant sampled cost.

## Profiler and inspector evidence

The existing extension profiler launcher was used without new game instrumentation.
The valid diagnostic fixture runs 6,000 simulation updates in batches of 120 per
`love.update`; its capture includes fixture reset/report work and is not a rendered
frame benchmark. Each capture has 51 frame records. The reference collected 295 CPU
samples over 3,291 ms; candidate collected 269 over 3,141 ms. Leaf samples in
`tuabehavior.lua` were 218 and 214 respectively, compared with 4 and 3 in collision
math. Sampling counts are diagnostic evidence, not timing percentages precise enough
to infer the candidate's speedup. `arena-game-08-profiles.json` retains the summaries,
VM state samples and capture metadata. Full reports remained in temporary output.

The profiler metadata reports target as unknown; the separately recorded compiler
provenance above supplies its target. Its manifest SHA identifies the emitted manifest,
not the temporary benchmark entry or the compiler. The measurement runner separately
hashes its authored harness and emitted combat modules. Captures preserve the dirty
pre-commit source revision, not a fabricated clean final revision.

The real inspector launcher/transport received the actual session module's playing
state, ECS world (32 sampled entities) and bullet pool (one active bullet at capture).
The snapshot's truncation flag was true: ECS component internals reached another
bound despite fewer than 100 entities. The guide explicitly explains those limits.
This was native localhost transport qualification, not an installed VS Code UI test.

## Correctness and commands

From the checkpoint:

```sh
../../../../../target/release/tua fmt
../../../../../target/release/tua fmt tests/reference-combat.tua src/locales/en.lua src/locales/fr.lua tests/*.lua
sh tests/check.sh
```

The formatting/type/build gate and all six inherited Lua suites passed. The new
comparison builds the reference in a disposable project and copies its emitted module
into an isolated test game. It never edits generated gameplay Lua. The differential
suite compares 3,600 mixed updates exactly across 8/32/128 enemies, including crowded
attack intents, projectile priority, component replacement, death and reset. Every
recorded timing/allocation trial also matched its reference outcome hash.

From repository root (use fresh output paths):

```sh
python3 examples/tutorials/arena-game/checkpoints/08-profile-the-game/tests/measure.py \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --output /tmp/tua-arena-c8-final-measure
node examples/tutorials/arena-game/checkpoints/08-profile-the-game/tests/profile.js /tmp/tua-arena-c8-candidate-v2.json
node examples/tutorials/arena-game/checkpoints/08-profile-the-game/tests/profile.js /tmp/tua-arena-c8-baseline-v2.json --baseline
node examples/tutorials/arena-game/checkpoints/08-profile-the-game/tests/inspector.js /tmp/tua-arena-c8-inspector.json
python3 examples/tutorials/arena-game/checkpoints/08-profile-the-game/tests/verify.py \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --output /tmp/tua-arena-c8-persistence
```

Both independent save-test processes passed. All five native replay scenarios passed
through `scripts/love_replay.py`, with the C8 project, selected compiler/LÖVE binaries,
`--fps 30 --seed 71`, and these scenario-specific arguments:

| Scenario | Frames | Output | Capture |
| --- | ---: | --- | --- |
| `tests/workload-replay.lua` | 45 | `/tmp/tua-arena-c8-native` | `--capture` |
| `tests/replay.lua` | 350 | `/tmp/tua-arena-c8-game` | `--video` |
| `tests/save-replay.lua` | 40 | `/tmp/tua-arena-c8-save` | `--capture` |
| `tests/dialogue-replay.lua` | 60 | `/tmp/tua-arena-c8-dialogue` | `--capture` |
| `tests/shader-replay.lua` | 50 | `/tmp/tua-arena-c8-shaders` | `--capture` |

Frame 15 of the workload replay was visually inspected and retained as
`docs/assets/tutorials/arena-game/08-workload.png`, beside provenance JSON. It shows
the dense enemy ring, health bars, shadows, fire feedback and HUD. The scenario also
returns to eight enemies with clean projectile state. Dense overlap is intentional
stress input, not a balanced or polished wave arrangement. Existing art/audio assets
are carried unchanged.

Docs gate: `make docs-build`, `make json-check`, `make diff-check`, explicit parsing
of new JSON evidence and staged whitespace checks. Full compiler/extension suites,
installed editor interaction, other platforms, JIT-enabled benchmarks, human playtests,
GPU timing and audio recording were not performed. Video remains silent. C9 owns the
remaining complete-path audit.
