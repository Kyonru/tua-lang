# Stealth maze Tua developer trial

Revision under test: `6bb460d01454dd186894f6d5279e46f74ac6f62b`, Tua 0.9.0, 2026-11-13. This is a compact, playable LOVE game, not a production benchmark. Set `TUA_BIN=/Users/kyonru/Documents/projects/Experiments/tlua/target/debug/tua` in your shell, then run from this directory after `$TUA_BIN build`: `/Applications/love.app/Contents/MacOS/love build`. Move with arrows/WASD, press Space near the guard to stun once, press R to restart. Reach the green exit. The map seed is 817. The audit fixture and other concurrent trial work were uncommitted/ignored during verification.

The trial deliberately crosses files and domain boundaries: a generic ledger stores a protocol-conforming inherited class instance; a seeded maze and FOV drive a guard behavior tree inside ECS; signals update a store and state machine; an object pool supplies movement effects; sprites, camera, shader, audio and input present the state.

## Verification summary

| Check | Exact command (from this directory unless stated) | Result |
| --- | --- | --- |
| Format | `$TUA_BIN fmt`; `$TUA_BIN fmt --check` | Pass, 6 files. The first format after a multiline condition needed a second pass; simplifying that condition stabilized it. |
| Lint | `$TUA_BIN lint` | Pass, 6 files. |
| Check | `$TUA_BIN check` | Pass, 6 files; TL5007 warning for a pooled spark acquired at main:182 and released by the ECS callback at main:133. |
| LOVE-aware check | `$TUA_BIN check --love /Applications/love.app/Contents/MacOS/love` | Pass with the same warning. |
| Build | `$TUA_BIN build` | Pass, 10 source files; copied PNG, OGG and GLSL into build; emitted opt-in helper requires. |
| Busted | `$TUA_BIN test --busted /opt/homebrew/bin/busted` | 4 successes, 0 failures/errors/pending. |
| Game boot | `TUA_MAZE_SMOKE=1 timeout --signal=KILL 12s /Applications/love.app/Contents/MacOS/love build` | GUI LOVE exit 0; `TUA_MAZE_SMOKE_OK 15 11 21 guard 0 13 8 true`. Shader loaded. |
| Play route | `TUA_MAZE_ROUTE_SMOKE=1 timeout --signal=KILL 12s /Applications/love.app/Contents/MacOS/love build` | GUI LOVE exit 0; `TUA_MAZE_ROUTE_RESULT won 20 true 20`. Calls the same movement and stun actions as player input. |
| Headless audio | In `repros/headless_audio`: `$TUA_BIN build`; `SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy timeout -k 2s 8s /Applications/love.app/Contents/MacOS/love build` | Exit 0; `TUA_AUDIO_SMOKE started started 2` from cue and music scene. This does not establish audible output. |
| LSP requests/edit | In `repros/factory_forward`: `python3 lsp_probe.py` | Completion has `level`, hover reports `() -> number`, definition points to actors.tua; editing provider to string and back changes hover accordingly. Protocol-level only, no visible VS Code timing. |
| Small-project timing | `python3 measure.py` | Five-sample medians: check 1435.5 ms, format-check 50.2 ms, build 1544.9 ms. Concurrent activity and a tiny project limit comparison. |

The first GUI smoke hung once for over a minute. A bounded replay and an independent copied-build replay exited normally. Cause of that one run is unknown; it is not classified as a defect. The game smoke confirms boot, shader load, four update/draw frames and a route to the win state, not a human playtest or complete audiovisual quality check. No installed VS Code host was exercised by this trial.

## Feature-by-feature coverage

Statuses: **runtime** means Busted or LOVE asserted observable behavior, **checked** means source/build only, **protocol** means actual LSP request, **inspected** means source/docs only, and **blocked** states the limit.

| Coverage item | Trial evidence and status |
| --- | --- |
| Typed locals, records, arrays, unions, optional/nil, aliases | `src/main.tua:7-11,61-83`, `src/game/rules.tua:5-6,25-27`; checked; `repros/negative` rejects wrong typed variable. |
| Functions, callbacks, multiple returns, closures, narrowing | `src/game/maze_logic.tua:4-35` uses Dice multiple returns and FOV callback; `src/main.tua:155-170` signal/store closures; runtime via 4 Busted tests and GUI smoke. Union narrowing is inspected, not specifically asserted. |
| Classes, inheritance, dispatch, aliases | `src/game/actors.tua:7-60`, `src/game/rules.tua:25-27`; `tests/maze_spec.tua` checks inherited `move`, overridden `label`, generic ledger, imported factory at runtime. A distinct alias upcast diagnostic defect is below. |
| Generic aliases/functions/classes, imported specialization | `src/game/actors.tua:46-60`, `src/game/rules.tua:6-27`, `src/main.tua:8-9`; checked, emitted Lua executed in Busted and LOVE. |
| Protocol conformance/bound across files | `src/game/actors.tua:2-4,26`, `src/game/rules.tua:12-15`; checked and runtime. `repros/negative` confirms missing-method TL1008. |
| Tua imports, plain Lua interop, exports | `src/game/rules.tua:1-3,36-42`, `src/game/legacy.lua`; multi-file check/build and Busted runtime. |
| LOVE callbacks, object methods, asset constructors | `src/main.tua:137,226-334`; GUI smoke confirms callbacks, `sound`, shader load, draw loop. |
| Sprite sheet/animation | `src/main.tua:40-49,176-181,253,320`; LOVE smoke draws idle sprite and route uses `play("walk")`; frame timing not independently measured; preview blocked. |
| Vec2/math | `src/main.tua:7,176-179`, `src/game/rules.tua:30-33`; route runtime moves from start to exit. |
| Signals | `src/main.tua:20,119,155-166`, `tests/interaction_spec.tua:7,34-46`; Busted runtime asserts handler payload and store update. |
| Stores | `src/main.tua:12-19,159-170`, `tests/interaction_spec.tua:8,34-52`; Busted runtime asserts state after signal. |
| State machines | `src/main.tua:21-29,158,186-192`, `tests/interaction_spec.tua:9-12,39,52`; runtime transition to alert and LOVE win. Invalid-event behavior not tested. |
| Typed input/Baton | `src/main.tua:31-38,235-251`; GUI boot initializes and polls input; automated route bypasses physical key events, so gameplay key response is not independently confirmed. |
| Tween | `src/main.tua:168-170,256,331-332`; alert event starts tween and GUI update runs it; alpha values not directly asserted. |
| Camera | `src/main.tua:50,165,257-258,302-325`; GUI draw uses follow and world transform; screen coordinate correctness not asserted. |
| Rot procedural helpers | `src/game/maze_logic.tua:4-35`; Busted asserts deterministic map, route and visibility; LOVE shows map and guard navigation. |
| Behavior trees | `src/main.tua:72-127`, `tests/interaction_spec.tua:20-32,41-59`; Busted tick, branch, cancel; LOVE smoke sees guard move, route sees chase/alert. Per-actor instance count is one; reset called on restart but no automated restart assertion. |
| ECS | `src/main.tua:54-60,109-135`, `tests/interaction_spec.tua:19,31-47`; Busted system update and LOVE guard update. |
| Pools | `src/main.tua:61-70,129-134,182-185`, `tests/interaction_spec.tua:13-18,53-57`; Busted acquire/release/count; GUI route reports 20 active sparks at immediate finish. Reuse after release not directly asserted. |
| Audio/AudioDesign | `src/main.tua:137-166`, `repros/headless_audio/src/main.tua`; cue and music scene start under headless LOVE, no audible verification. |
| ShaderScan/GLSL | `src/main.tua:51,229-232,260,302-306`, `src/shaders/fog.glsl`, `src/shaders/tint.glsl`; GUI smoke confirms shader object present. Hot reload was not triggered by a file edit; compiled-path caveat below. |
| Asset metadata/navigation | Real PNG, OGG, GLSL in `src/assets` and `src/shaders`; build copies assets. Asset hover/navigation not exercised. |
| fmt/lint/check/build/emitted Lua | Commands above. Emitted main.lua starts with 13 opt-in helper requires; runtime boots and wins. |
| Busted testing | 4 game assertions in `tests/maze_spec.tua` and `tests/interaction_spec.tua`; pass. |
| Completion/hover/signature/definition | `repros/factory_forward/lsp_probe.py` exercises completion, hover and definition. Signature help not exercised; installed-host visibility blocked. |
| Previews/cross-view navigation | Inspected docs and feature source only. No installed VS Code host/preview verification in this child trial. |
| Save/edit/recovery | LSP `didChange` updates method hover number→string→number in same server. No filesystem event replay here. |
| Performance | Five CLI measures and protocol requests. No trigger-to-widget or large-project evidence. |

## Confirmed defects and friction

### MAZE-01 — Imported subclass alias rejected as a base array element

Run `$TUA_BIN check` in `repros/inheritance_alias`: exit 1, `main.tua:10:37 error TL1003: type mismatch: expected ActorInstance, found GuardInstance`. The imported `Guard` extends `Actor`; a base-typed array should accept it. The same array with `Actors.Guard.new(...)` passes; removing the unrelated imported protocol-bound generic function also passes. This points to context-sensitive alias/subtype identity resolution rather than runtime inheritance. Workaround in game: construct directly for the array at `src/game/rules.tua:26`. Parent independently reproduced and owns the product fix/backlog.

### MAZE-02 — Imported class receiver falsely rejected despite correct LSP resolution

Run `$TUA_BIN check` in `repros/factory_forward`: exit 1, `main.tua:3:15 TL1003: type mismatch at self: expected Guard, found Guard`. The provider returns a `GuardInstance`; even direct imported `Actors.Guard.new()` in the consumer can trigger the same diagnostic. A local class control passes, and explicit `local guard: GuardInstance` annotation passes. `python3 lsp_probe.py` confirms completion `level`, hover method return number, and definition to `actors.tua`. The editor and checker disagree on a value that executes correctly under Lua runtime control. Root cause is likely imported declaration identity or receiver comparison; precise internal path needs investigation. Parent independently reproduced. Workaround: annotate receiver local.

### MAZE-03 — Shader paths change meaning across source and generated game

Loading `src/shaders/fog.glsl` from source appears natural, but built `main.lua` runs under `build/`, where only `shaders/fog.glsl` exists. On GUI LOVE, `ShaderScan:load_shader("fog", "src/shaders/fog.glsl")` reports an include/read error and shader remains absent; changing to `shaders/fog.glsl` makes smoke output final field `true`. Include line in fog shader also uses built-root `shaders/tint.glsl`. This is a portability/documentation friction point, not a compiler bug in isolation. A source-aware asset helper or clearer generated-layout docs would reduce trial and error.

### MAZE-04 — Pool lifetime warning is noisy for valid ECS ownership transfer

`$TUA_BIN check` warns TL5007 at `src/main.tua:182`: spark acquired but no later same-file release of the same local. `sparkSystem` releases the same object at line 133 when its ECS entity expires; the warning accurately describes its syntactic limit, yet suggests a leak during a common pool→ECS ownership pattern. A cross-function ownership annotation or narrower wording would help. Runtime Busted validates release; GUI route finishes before sparks age out. Do not suppress this warning globally based on this case alone.

### MAZE-05 — Formatter needed a second pass on a multiline condition

An initial `$TUA_BIN fmt` succeeded, then immediate `$TUA_BIN fmt --check` reported `would reformat src/main.tua`. The source had a multiline `if Rules.pointDistance({..},{..}) > 3 then return end`, and formatter indentation drifted through following declarations. Replacing it with a named distance local allowed three repeated `fmt`→`fmt --check` cycles to pass. This is a confirmed symptom from this trial, though no preserved minimal formatter repro remains and exact parser cause is unverified. It should be investigated before classing as a product regression.

## Developer experience notes

First paths were `docs/games.md` and feature examples, then actual project checks. The lower-camel factory and type-only import workflow was reasonably discoverable, but imported class identity diagnostics required an explicit local annotation and manual isolation. The `expected Guard, found Guard` text is especially unhelpful because it hides declaration provenance. Rot + BehaviorTree + ECS composition was direct once each helper's update ownership was clear; using `any` for ECS Brain fields at `main.tua:55` loses useful type information at that boundary. The game used a typed `GuardContext` cast inside the system to recover it.

The shader source/build root distinction caused a live failure before correction. The pool warning remained after correct runtime release. These are the most tangible game-authoring costs. The route and headless audio runs show the chosen mechanics execute, while sprite timing, key event handling, hot reload, preview usefulness, visible editor latency and long-session behavior remain unqualified by this trial.

## Reproduction entry points

- `repros/inheritance_alias`: alias upcast failure, with source controls documented above.
- `repros/factory_forward`: checker/editor mismatch and `lsp_probe.py` for real LSP messages.
- `repros/negative`: intentional missing protocol method and wrong typed assignment.
- `repros/headless_audio`: audio cue and music-scene runtime smoke without graphics.
- `measure.py`: five-sample CLI timing script. It uses the local binary; do not compare across machines without controlling build and workload.

Build products are intentionally not retained in this report directory. Re-run `tua build` to regenerate `build/`.
