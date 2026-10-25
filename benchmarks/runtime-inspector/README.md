# Runtime Inspector performance

Compare a fresh game without the inspector against a game that was inspected
and then disabled, a paused inspector, and active inspection at two snapshots per
second. These are different states: disabling inspection before launch already
means **no shim**; disabling it afterward leaves runtime wrappers in that process
until it exits.

[Original overhead comparison](2026-10-25-m3-max.md) ·
[Reconnect, memory, rendering, and expanded-tree validation](2026-10-25-validation.md).

## Run the benchmark

From `editors/vscode`:

```sh
TUA_BENCH_OUTPUT=/tmp/tua-inspector-benchmark.json \
  node scripts/runtime-inspector-benchmark.js
```

Set `TUA_LOVE` to a LÖVE executable outside the default macOS installation.
`ps` is used for process RSS measurements on macOS/Linux. The benchmark creates
and removes its own temporary project; it does not build or modify your game.

Defaults: three repetitions per case, three seconds of warmup, eight seconds of
measurement, and a fresh process for every run. Mode order rotates between
repetitions. Optional settings:

| Variable | Default | Purpose |
| --- | --- | --- |
| `TUA_INSPECTOR_ASSETS` | Extension assets | Override shim directory for paired implementation comparisons |
| `TUA_BENCH_SECONDS` | `8` | Measurement duration per run |
| `TUA_BENCH_WARMUP` | `3` | Warmup duration per run |
| `TUA_BENCH_REPETITIONS` | `3` | Repetitions per workload and mode |
| `TUA_BENCH_PROFILES` | `light,heavy` | Workloads to run |
| `TUA_BENCH_MODES` | All four | Comma-separated modes listed below |
| `TUA_BENCH_MAX_LOAD` | Half the logical CPU count | Refuse to start a case above this one-minute host load |
| `TUA_BENCH_ALLOW_BUSY` | Unset | Set to `1` to collect explicitly noisy measurements anyway |

Reports include per-frame samples, host loads, source hashes, warnings, and a
completion flag. Interrupted or failed comparisons retain their completed runs.
Do not treat a low snapshot rate or disconnected inspector as a cheap active run.

## Cases and workloads

| Mode | Shim | Runtime wrappers | Snapshot updates |
| --- | --- | --- | --- |
| `no-shim` | Not loaded | None | None |
| `disabled-after-launch` | Loaded, then disconnected | Remain installed | None after warmup |
| `paused` | Loaded and connected | Remain installed | Paused after connection |
| `active` | Loaded and connected | Remain installed | Twice per second |

Both deterministic workloads use the actual bundled runtime modules. Each frame
updates entity positions and every store, emits signals, and acquires/releases
32 pooled objects. Four state machines alternate state once per 60 frames.

| Workload | Entities | Stores | Fields per store | Signal emissions per frame |
| --- | ---: | ---: | ---: | ---: |
| Light | 200 | 8 | 16 | 8 |
| Heavy | 2,000 | 48 | 32 | 128 |

The inspector retains its production sampling and serialization limits. Heavy
snapshots may therefore be truncated; the report records that count.

## What the measurements mean

- **Frame work:** wall time for the deterministic update and the real inspector
  tick that follows it. Pacing sleep is excluded. Tail percentiles expose periodic
  snapshot work, garbage collection, and OS scheduling delays.
- **Game CPU:** process CPU time across the measurement window, including runtime
  housekeeping. It excludes time when the process is descheduled or asleep.
- **Lua heap:** sampled allocated memory, plus retained memory after a full GC.
  The benchmark's timing arrays and formatted result strings are also included;
  this is not an isolated accounting of inspector allocations.
- **Process RSS:** resident memory sampled once per second; includes LÖVE and
  system libraries, not just inspector allocations.
- **Node CPU:** listener, parsing, source lookup, presentation building, launcher
  reconciliation, and benchmark sampling. It is not isolated inspector CPU and
  does not include VS Code tree layout or painting.

The fixture is paced near 60 Hz but performs no game rendering. This measures
runtime overhead, **not your game's FPS, GPU cost, startup latency, or the cost of
an expanded editor sidebar**. Run comparisons on a quiet machine and inspect the
ranges between repetitions before interpreting small differences.

## Extended validation

Run from `editors/vscode`; each command creates isolated temporary projects.
Native benchmarks require LÖVE. The UI benchmark requires a graphical session
and a local VS Code/VSCodium executable (`VSCODE_EXECUTABLE_PATH`).

```sh
TUA_SOAK_OUTPUT=/tmp/inspector-soak.json node --expose-gc scripts/runtime-inspector-soak.js
TUA_UI_OUTPUT=/tmp/inspector-ui.json node scripts/runtime-inspector-ui-benchmark.js
TUA_GAME_OUTPUT=/tmp/inspector-game.json node scripts/runtime-inspector-game-benchmark.js /path/to/zombie
```

- **Soak:** ten minutes by default (`TUA_SOAK_SECONDS`), with a simultaneous
  no-shim control, full-GC Lua/Node heap samples, ephemeral store churn, forced
  TCP disconnects every 30 seconds, pause/refresh/resume, live frequency changes,
  and explicit disable. Run with `--expose-gc` for comparable Node retained heaps.
  Larger durations can test multi-hour sessions; the recorded run is ten minutes.
- **Native tree:** 16 stores × 128 expanded values; paused and live phases,
  keyboard-to-selection latency through CDP, extension-host timer lag, and tree
  construction time. `TUA_UI_SECONDS` defaults to 15 per phase.
  `TUA_PRESENTATION_MODULE` can point at an older presentation module for pairing.
  Keep the test window focused; do not run visible games concurrently.
- **Rendered game:** specifically uses the zombie project's `game.range`,
  `player.gun`, and `core.state` APIs. Copies source/config, builds with the bundled
  compiler, then runs deterministic aiming and five shots per second at 960×540.
  Three fresh-process runs per no-shim/all-category/store-only condition, with
  180 warmup and 600 measured frames. Visible, vsync off, paced at 60 Hz. This is
  a controlled rendering scenario, not a full gameplay or uncapped FPS benchmark.

For a paired snapshot comparison, copy current `assets/inspector` into a temporary
folder and replace its `snapshot.lua` with the pre-optimization revision
(`62eaae6:editors/vscode/assets/inspector/snapshot.lua`). Alternate that folder and
current assets through `TUA_INSPECTOR_ASSETS`, using heavy/active, one repetition
per invocation, three pairs. Source hashes are included in each output.
