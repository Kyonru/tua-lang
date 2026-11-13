# Three-game Tua developer-experience audit: coverage contract

Baseline: `6bb460d0`, 2026-11-13, clean tracked worktree. These are simulated developer trials, not human usability studies. Each agent owns one game in `trials/` and must record actual use, exact commands, outcomes, and what was awkward. A passing `tua check` alone does not prove a feature achieved its intended game behavior.

Status key: **R** = observable behavior asserted in LÖVE, Lua, or Busted; **C** = source checked/built or negative diagnostic exercised; **L** = real LSP protocol request; **I** = inspected only; **P** = partial coverage, with the missing aspect identified in the trial report. A cell may contain several statuses because a feature's parts were exercised at different layers. Trial evidence: [arena](trials/arena/REPORT.md), [alchemy](trials/alchemy/REPORT.md), [maze](trials/maze/REPORT.md). No cell implies installed VS Code visual verification.

| Feature / workflow | Arena defense | Alchemy | Stealth maze | Expected evidence |
| --- | --- | --- | --- | --- |
| Typed locals, records, arrays, unions, optional/nil, aliases | C, R, P | C, R | C, R, P | Positive and intentionally wrong type checks |
| Functions, callbacks, multi-returns, closures, narrowing | C, R, P | C, R | C, R, P | Check + runtime result or unit assertion |
| Classes, inheritance, virtual dispatch, instance aliases | C, R, P | C, R | C, R, P | Base-typed runtime dispatch |
| Generic alias, function, class and imported specialization | C, R, P | C, R | C, R, P | Cross-file checked use and emitted Lua |
| Protocol conformance and protocol bound across files | C, R, P | C, R | C, R, P | Positive and missing-method diagnostics |
| Tua imports, plain Lua interop, project exports | C, R | C, R | C, R | Multi-file build and Lua load |
| LOVE callbacks, object methods, asset constructors | C, R, P | C, R, P | C, R, P | Build and LOVE runtime where available |
| Sprite sheet and animation | C, R, P | C, R, P | C, R, P | Frame selection/timing and preview |
| Vec2 and game math | C, R | C, R | C, R | Observable movement/geometry |
| Signals | C, R, P | C, R | C, R | Emitted and handled event with payload |
| State stores | C, R | C, R | C, R | State update/subscription behavior |
| State machines | C, R, P | C, R, P | C, R, P | Transition and invalid-event behavior |
| Typed input / Baton | C, R, P | C, R, P | C, R, P | Action binding and gameplay response |
| Tweening | C, R | C, R, P | C, R, P | Time-dependent observable value |
| Camera | C, R, P | C, R | C, R, P | World/screen coordinate or follow behavior |
| Rot procedural helpers | C, R | C, R, P | C, R | Seeded deterministic map/path/dice use |
| Behavior trees | C, R | C, R, P | C, R, P | Tick, branch, per-actor state, reset |
| ECS | C, R | C, R | C, R | Component schema and system update |
| Object pools | C, R, P | C, R, P | C, R, P | Acquire/release/reuse and counts |
| Audio cues, scenes, AudioDesign | C, R, P | C, R, P | C, R, P | Cue generation/routing; playback if LOVE available |
| ShaderScan, GLSL includes, uniforms, hot reload | C, R, P | C, R | C, R, P | Check, generated Lua, validator/reload if available |
| Asset metadata and source navigation | C, R, P | C, R, P | C, R, P | Real image/font/audio/GLSL paths |
| `tua fmt`, `lint`, `check`, `build`, generated Lua | C, R | C, R | C, R | Command logs, output inspection |
| `tua test` / Busted | R | R | R | Meaningful game rule test |
| Completion, hover, signature, definition during edits | L, P | L, P | L, P | Exact LSP requests/responses or installed host; protocol-only labeled |
| Feature previews and cross-view navigation | I, P | I, P | I, P | Real host if possible, otherwise inspected/blocked |
| Save/edit/recovery, dependency invalidation | L, R, P | L, R, P | L, P | Recheck after source change |
| Performance: cold/warm suggestion, check/build, format | C, L, P | C, L, P | C, P | Workload, repeated measures, host limits |

Toolchain-only workflows were handled separately: portable `.love` package, `tua init`, and `tua actions init` were exercised by the lead; web/custom packaging, the generated GitHub workflow on GitHub, runtime inspector, API/MCP bridge, and source-mapped debugging were not run. They are qualification gaps, not implied passing features. See [VERIFICATION.md](VERIFICATION.md).

The lead reconciled overlapping symptoms and independently reproduced every High finding. The matrix is broad but deliberately does not claim every subfeature was proven; each `P` and each protocol-only result is explained in the corresponding trial report.
