# Arena defense: Tua 0.9.0 developer trial

## Scope and result

Revision `6bb460d0` was the tracked baseline. This is a small, playable LÖVE arena: WASD moves a defender, Space or mouse attacks nearby invaders, each invader has a behavior-tree runner, hits change a store and signal-driven state machine, and the draw loop uses a sprite animation, camera, shader, and audio cues. Run from this directory with `tua build` then `love build` (or `/Applications/love.app/Contents/MacOS/love build` on this machine). Generated files are intentionally omitted. This is a simulated developer trial, not a usability study.

Final `tua fmt --check`, `tua check`, `tua lint`, `tua build`, and `tua test` passed. The five Busted tests passed. An earlier bounded 90-frame actual LÖVE run exited 0 with `ARENA_SMOKE_OK 90 0 1`; it exercised load, update, draw, shader compilation, generated audio cue playback status, event delivery, and tween decay. A separate 180-frame LÖVE run changed the built shader include and printed `[ShaderScan] reloaded ring (assets/glow.glsl)` and `ARENA_HOT_RELOAD_OK 180`. A later clean-source graphical-host replay timed out before any application output, and the plain Lua host control also timed out; see the recheck below. The successful runs did not exercise physical keyboard/mouse input, preview webviews, or visible completion menus. Only protocol-level LSP requests were measured.

The game was straightforward to assemble after finding the domain examples. The most consequential discovery was a compiler miscompile of a one-line subclass initializer: a `super(); self.speed = 120` constructor yielded speed 0, while the multiline form yielded 120. A second confirmed defect makes an imported subclass-to-base assignment fail only after a protocol-typed array is used. Editor support kept the imported generic field's member completion, but hover on the nested `value` returned null. Cross-file signal events also lost payload checking even when signature help could infer the payload through a local alias.

## Reproduce and verification log

Use `/Users/kyonru/Documents/projects/Experiments/tlua/target/debug/tua` in place of `tua` if it is not on `PATH`. Paths below are relative to this `arena` directory unless a `cd` is shown.

| Command / request | Final observation |
| --- | --- |
| `tua fmt --check` | exit 0, 5 files |
| `tua check` | exit 0, 5 files; warning TL5007 at `src/game/core.tua:126:9` |
| `tua lint` | exit 0, 5 files |
| `tua build` | exit 0, 11 files; same warning |
| `tua test` | exit 0, `5 successes / 0 failures / 0 errors / 0 pending` |
| `lua -e 'package.path="build/?.lua;build/?/init.lua;" .. package.path; local g=require("game.core"); assert(g.spawnRoute and #g.spawnRoute > 0); local p=g.world:get(g.player,g.Position); assert(p and p.x==320); g.spawnEnemy(330,240); assert(g.attack()); assert(g.state:get().score >= 5); g.update(1/60); print("ARENA_CORE_OK",g.state:get().score,g.sparks:activeCount())'` | `ARENA_CORE_OK 5 1`; this checks generated Lua without LOVE graphics/audio |
| `TUA_ARENA_SMOKE=1 /Applications/love.app/Contents/MacOS/love build` | Earlier run: exit 0, `ARENA_SMOKE_OK 90 0 1`; later graphical-host replay timed out at 20 s with no output. Both used local host access outside default sandbox. |
| `python3 lsp_probe.py > lsp-results.jsonl` | Actual stdio LSP completion/hover/signature/definition replies; no VS Code widget paint measurement |
| `cd repros/protocol-negative && tua check` | exit 1, TL1008: `Missing` lacks `Tickable.update` across files |

The hot-reload smoke uses `TUA_ARENA_HOT_RELOAD_SMOKE=1` while the built `assets/pulse.glsl` is changed from `0.75` to `0.70` after the game launches; the test restores the built file. The source include is never changed by this probe. The game asserts that `shaders.s.ring` is a new shader object at frame 180. For a repeatable command without manual timing, use:

```sh
python3 - <<'PY'
import os, subprocess, time
from pathlib import Path
p = Path('build/assets/pulse.glsl')
original = p.read_text()
env = os.environ.copy()
env['TUA_ARENA_HOT_RELOAD_SMOKE'] = '1'
game = subprocess.Popen(['/Applications/love.app/Contents/MacOS/love', 'build'], env=env,
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
try:
    time.sleep(.7)
    p.write_text(original.replace('0.75', '0.70'))
    out, err = game.communicate(timeout=18)
    print(game.returncode, out, err)
finally:
    p.write_text(original)
    if game.poll() is None: game.kill()
PY
```

Three local warm runs of the same small project yielded `check` 4607/4619/4619 ms (median 4619), `build` 7072/7355/7149 ms (median 7149), and `fmt --check` 48/46/48 ms (median 48). These are wall-clock command timings on a busy development host, not isolated benchmarks or visible editor latency. The first direct imported generic completion in one fresh LSP process took 1167 ms; the subsequent local-alias completion took 314 ms. Hover replies took 210–349 ms and one signature request 100 ms. The trace contains one sample per request and request ordering affects warmness; no performance target can be concluded from it.

## Feature ledger

**Used** means source was built and the named test, generated-Lua check, or LÖVE run exercised the stated behavior; **attempted** means a concrete test exposed a problem; **inspected** means no live verification. Source links use paths in this trial.

| Coverage row | Status and evidence |
| --- | --- |
| Typed locals, records, arrays, unions, optional/nil, aliases | **Used**: `src/game/core.tua:5–19,109–120`, `src/main.tua:26,30`; checked and built. `if not position then return end` narrows an optional ECS lookup (`core.tua:133–139`). Explicit wrong imported base assignment is **attempted** in `repros/imported-base` (GAME-01). |
| Functions, callbacks, multiple returns, closures, narrowing | **Used**: generic `result` at `core.tua:12`, closure store watcher at 50, ECS callbacks at 66/86/101, multiple return `local x, y` in `main.tua:79`; check, tests, LÖVE smoke. A Busted `assert.is_not_nil(pos)` did not narrow `pos.x`; replacing it with `if not pos then error(...) end` worked. Assertion narrowing is a capability gap unless promised elsewhere. |
| Classes, inheritance, virtual dispatch, instance aliases | **Used with defect**: `actors.tua:6–50`, base array `core.tua:18`, Busted `tests/core_spec.tua:4–11` proves 0.5 versus 1.0 progress through base type. Initial one-line initializer silently miscompiled; see GAME-08. |
| Generic alias, function, class, imported specialization | **Used**: `Result<T>` and `result<T>` at `core.tua:10–19`; `Slot<T>` and `tickOne<T:Tickable>` at `actors.tua:37–46`; imported `selected.value.speed` at `core.tua:157–159` moves the player in Busted. Nested field hover is **attempted** and fails; completion works. |
| Protocol conformance and bound across files | **Used with defect**: `actors.tua:1–4,21–35,43–46`; `core.tua:163`; cross-file missing method yields TL1008 in `repros/protocol-negative`. Protocol array use breaks unrelated subclass assignment in `repros/imported-base` (GAME-01). |
| Imports, Lua interop, project exports | **Used**: `core.tua:1–3,66–69`, `src/legacy.lua:1–9`; generated Lua module `require('game.core')` and headless assertion pass. |
| LOVE callbacks, object methods, constructors | **Used**: `src/conf.tua:1–6`, `src/main.tua:51–137`; actual 90-frame LÖVE smoke. Physical input was not driven. |
| Sprite sheet and animation | **Used for load/update/draw**: four frames, uneven durations and origin at `main.tua:3–11`, update 85, draw 116; LÖVE smoke. Frame-by-frame visual order and preview were **not verified**. |
| Vec2 and math | **Used**: `core.tua:92–93,138–139,182–185`; Busted player movement and route test at `tests/core_spec.tua:13–24`. |
| Signals | **Used with cross-file gap**: handlers and payload at `core.tua:54–64`, emit at 97/146, hit-signal LÖVE smoke at `main.tua:62–72`; Busted hit test 51–56. Wrong cross-file payload is accepted; see GAME-SIGNAL. |
| State stores | **Used**: `core.tua:41,50–64`; score and HP observable in Busted 26–33 and 51–56; watcher stores latest HP via `lastHp`. |
| State machines | **Used**: `core.tua:42–48,59`; Busted transition to `lost` and attack guard at 51–56. Invalid-event behavior was not separately asserted. |
| Typed input / Baton | **Used in LÖVE update path**: mappings at `main.tua:13–22`, `controls:update/get/pressed` 78–83. Actual key/mouse injection and response **not verified**. |
| Tweening | **Used**: `main.tua:27,66–67,86`; after 90 LÖVE frames alpha is at most .01. |
| Camera | **Used**: `main.tua:24,65,90–91,108–125`; LÖVE draw loop and follow/shake calls pass. Visual camera framing not captured. |
| Rot procedural helpers | **Used**: seeded RNG, dice, generated maze, A* route, color at `core.tua:110,145,191–193`; route asserted at `tests/core_spec.tua:13–14` and headless Lua. First tried `Rot.Map.arena` for route, got nil because arena's cells are nonwalkable; docs led to `Rot.Map.generate`. This was fixture misuse, not a product defect. |
| Behavior trees | **Used**: `core.tua:71–99,115–120,140–142`; separate runner per enemy, reactive selector, hit and cooldown proven by Busted 35–49. Cancellation on enemy destruction exercised. |
| ECS | **Used**: component schemas/world at `core.tua:21–28`, systems 66/86/101, tick at 165–167; Busted movement, attack, AI and effects. |
| Object pools | **Used with warning**: pool/reset at `core.tua:30–38`, acquire 126, release 105; Busted active count 1 then 0 at `tests/core_spec.tua:26–33`. TL5007 warns even though the ECS effects system releases the item; see GAME-POOL. |
| Audio cues, scenes, AudioDesign | **Used**: generated `AudioDesign.render` cue, buses, music scene at `main.tua:30–49`, play and music state 54/63/82. LÖVE smoke asserts `setMusicState` started and generated cue status started/stolen. Audible quality was not assessed. |
| ShaderScan, include, uniform, hot reload | **Used**: `src/assets/glow.glsl:1`, `main.tua:25,56–61,87,111–116`; actual LÖVE shader compile and hot-reload object replacement pass. Path documentation issue below. |
| Asset metadata/source navigation | **Used for runtime loading only**: real PNG/OTF/WAV/OGG/GLSL paths at `main.tua:3,31,44,53,57`; build copied assets and LÖVE loaded them. Metadata hover or navigation was **not checked**. |
| Format, lint, check, build, generated Lua | **Used**: commands above; inspected emitted `main.lua`, `game/core.lua`, and bundled helper requires. `AudioDesign.render` became `require('tua.audio.design')` without source require; the helpers referenced by this game were bundled. Unused-helper exclusion was not separately tested. |
| Native tests / Busted | **Used**: `tests/core_spec.tua:3–57`, five passing behavioral tests. |
| Completion, hover, signature, definition during edits | **Attempted**: stdio LSP unsaved-buffer `didChange` requests in `lsp_probe.py`; raw results in `lsp-results.jsonl`. Imported generic completion and imported `Game.attack` definition succeed, nested field hover and direct module-property signal signature fall short. Installed VS Code menu/paint not measured. |
| Feature previews/cross-view navigation | **Inspected only**: repo docs describe animation, machine, behavior tree, ECS, and shader views, but this trial did not open the fixture in a VS Code extension host; do not count as preview UX verification. |
| Save/edit/recovery, dependency invalidation | **Attempted in protocol**: repeated unsaved full-buffer `didChange` requests and LSP replies in `lsp_probe.py`; fixture changed a built shader include and LÖVE reloaded. Renaming/deleting a dependency and real filesystem-event replay were not checked. |
| Performance | **Attempted**: command and protocol timings above; no key-to-menu or format-on-save measurement, so visible responsiveness remains unverified. |

## Findings and DevEx details

### GAME-01 — Imported subclass-to-base check changes after protocol-array use

**Confirmed correctness defect, high impact; independent lead recheck.** In `repros/imported-base`, `tua check` reports TL1003 at `src/game/core.tua:6:29`: `type mismatch at base: expected ActorInstance, found DefenderInstance`. `named` was already checked as `DefenderInstance`; `local direct: ActorInstance = Actors.Defender.new()` is accepted. Temporarily remove only the `tasks: Tickable[]` declaration and the `tasks` return field; the check succeeds. Importing the protocol without using it also succeeds. Expected: assigning a subclass instance to its imported base remains valid independent of a separate protocol-typed array. The order-sensitive behavior suggests shared type identity/provenance state, not a simple unsupported conversion. A regression should check direct and named subtype assignments before and after protocol usage across files; no product fix was made in this lane.

### GAME-08 — Inline subclass initializer assignment disappears from emitted Lua

**Confirmed compiler/runtime defect, high impact; independent lead recheck.** `repros/init-order/src/main.tua:8–20` contains `FastInline.init() { super(); self.speed = 120 }` and an equivalent multiline `FastMultiline.init`. Run `cd repros/init-order && tua fmt --check && tua check && tua build && lua -e 'package.path="build/?.lua;" .. package.path; local m=require("main"); print(m.inline,m.multiline)'`. Formatting and checking pass; runtime prints `0 120`. The generated `FastInline_new()` returns a table with inherited `speed=0` and never calls `FastInline_init`; the multiline constructor calls init. Expected both speed values 120. The source contract in `docs/classes.md:424–450` promises preservation of initializer order, and formatting should not change behavior. This first appeared as a trial enemy/player speed bug. Trial workaround: make `Actor.speed` required, set it in `Actor.init(speed)`, and call `super(120)`/`super(35)`; see `actors.tua:8–11,21–35`. A regression must compare inline/multiline emitted Lua and runtime state, including inherited defaults and multiple statements.

### GAME-SIGNAL — Cross-file event payload loses checking; direct module property loses signature

**Confirmed type-checker and editor gap, medium impact; protocol evidence.** `repros/event-signature/src/events.tua` registers `events:on('hit', function(damage:number) ...)`; `src/main.tua` imports the returned bus and emits a number. Replace `1` with `'bad'` in `src/main.tua:3`, run `tua check`: exit 0. The same wrong payload inserted as `events:emit('hit','bad')` in `events.tua` produces TL1003 `at damage: expected number, found string`. The checker should use the exported event schema across files or explicitly document the boundary. `python3 repros/event-signature/probe.py` requests signature help: same-file shows `damage: number`, imported local alias shows `value1: number`, but direct `EventModule.events:emit` falls back to `name: string, ...: any`; exact replies are in `repros/event-signature/lsp-results.jsonl`. A shared module-property signal schema should drive both checker and signature help. This is separate from GAME-01.

### GAME-HOVER — Imported generic field completion and hover disagree

**Confirmed protocol-level editor gap, medium impact; lead independently rechecked offsets.** The unsaved probe appends `local trialActor = Game.selected.value` to `src/main.tua` (`Game` imports `game.core`; `selected` is `Slot<DefenderInstance>`). At 0-based characters 35 and 37 on `value`, `textDocument/hover` returns null. Hover on `Game.selected` shows `Slot<DefenderInstance>`, hover on `trialActor` shows `Defender`, and completion on `Game.selected.value.` offers `update`, `speed`, `progress`, `elapsed`. Expected hover at the value field to show the specialized instance type. Compare `lsp_probe.py:89–111` and `lsp-results.jsonl`. A regression should request hover, completion, and definition for the same nested imported generic field after an unsaved edit. Installed editor rendering was not tested.

### GAME-POOL — Lifetime warning cannot follow ECS component transfer

**Confirmed warning quality gap, low/medium impact.** TL5007 at `core.tua:126` says acquired `flash` lacks a later same-file `sparks:release(flash)`. The flash becomes a `Spark` component at line 128 and is released as `spark` in the effects system at 101–107; Busted observes active count return to zero after update (`tests/core_spec.tua:26–33`). The warning describes a lexical heuristic rather than an actual leak. A fix should avoid false certainty when ownership escapes through a component, or explain that the analysis cannot follow transfers. A regression should preserve a true-leak warning while treating a proven ECS transfer accurately.

### GAME-SHADER-DOC — Source-style shader paths fail in built LÖVE game

**Documentation / example usability issue, medium impact.** The first shader attempt followed `examples/features/src/shaders.tua` and used `shaders:load_shader('ring','src/assets/glow.glsl')` with `#include 'src/assets/pulse.glsl'`. `tua build` succeeded, but `love build` reported `cannot read shader include "src/assets/glow.glsl"` because build output copies assets under `assets/`. Changing both to `assets/...` made the actual LÖVE shader load and hot reload pass (`main.tua:57`, `src/assets/glow.glsl:1`). The docs should give a path example that works for the common built-game invocation and clarify any source-root versus built-root distinction. A fixture test should load an include from the bundled game, not only check shader parsing.

## First-use path and workarounds

I began with `examples/features/src/{ecs,signals,store,state_machine,behavior_tree,object_pool,shaders,sprites,audio}.tua` and the corresponding feature docs. For protocol/generic syntax I used the class, generic, and protocol sections of `docs/classes.md`. The examples made the native helper names discoverable. I initially chose `Rot.Map.arena` expecting a walkable route; `Rot.Path.astar` returned nil. Reading the map docs showed the arena cells are walls, so `Rot.Map.generate(..., 'sidewinder')` solved the fixture. The shader example's `src/assets` path was misleading for the built LOVE runtime. The class initializer problem required emitting and running Lua to isolate; checker and formatter gave no clue. The cross-file type error on `local base: ActorInstance = named` was misleading because the direct constructor form passed; the smallest repro revealed its dependence on protocol array use. For a deferred shader variable, `local firstShader = nil` inferred nil and rejected the later assignment; `local firstShader: Shader | nil = nil` worked and is appropriate explicit typing.

## Limits and follow-up

No installed VS Code host was used for this trial. The LSP results prove server replies only, not menu opening, completion ranking/insertion behavior, hover rendering, or navigation UX. The LÖVE smoke proves update/draw and API calls, not visual quality or actual player input. The hot-reload probe edits copied built GLSL rather than a source file and therefore does not qualify `tua dev` rebuild/invalidation. Audio status proves a cue was accepted for playback, not what a listener hears. Small-project timing cannot qualify large projects or a long session. Asset hover/navigation, preview views, invalid state-machine events, full keyboard behavior, and dependency deletion/recovery need separate host-driven checks. No product files were changed by this trial.

### Clean-source LÖVE host recheck, later on 2026-11-13

After removing generated output, `tua build` again exited 0 (`built 11 file(s)` and the existing TL5007 warning). A Python `subprocess.Popen` replay of `/Applications/love.app/Contents/MacOS/love <absolute arena/build>` with `TUA_ARENA_SMOKE=1` was bounded to 20 seconds and killed as a process group on timeout. It produced no stdout or stderr before being killed. To isolate the host, `repros/love-host-smoke/main.lua` contains a plain Lua three-frame LOVE program that prints at module load, `love.load`, and update; that also timed out after 12 seconds without output. No LOVE process remained after either test. The executable responded normally to `--version` (`LOVE 11.5`). Setting `SDL_VIDEODRIVER=dummy` and `SDL_AUDIODRIVER=dummy` produced the expected OpenGL-window creation error and still required timeout cleanup; it is not a viable graphics-run substitute. The minimal control makes the current failure a graphical-host qualification blocker, not evidence of an arena/Tua runtime regression. The earlier successful LÖVE and shader-reload evidence remains valid for those earlier runs, but reproducibility in the current macOS GUI session is unverified.

`src/assets/music.ogg` was synthesized locally for this fixture using `ffmpeg`; it was not copied from a game or external music source. `ffprobe` reports a 4.000-second, 7,713-byte OGG file. SHA-256: `79f97da545f54f72c814884a589d088e88aed40c807c19b201c1c0655d6c70a4`.
