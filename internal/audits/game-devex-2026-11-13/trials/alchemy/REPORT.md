# Alchemy game developer-experience trial

**Revision:** `6bb460d0` baseline, Tua CLI `0.9.0` (`target/debug/tua`). **Scope:** this directory only; no product changes. The game is a small playable LÖVE program: Q/E choose the left element, A/D choose the right, and Enter or Space brews or continues. Four elements yield six unordered recipes. Brewing updates score, shows a tweened banner, plays a generated cue, shakes the camera, and emits pooled ECS sparks. An assistant behavior tree offers a hint. Source: [`src/main.tua`](src/main.tua), [`src/recipes.tua`](src/recipes.tua), [`src/model.tua`](src/model.tua).

The trial is an automated developer simulation, not a human playthrough. `love.load`, updates, draw, asset loads, shader compilation, a recipe event, pool release, and include hot reload ran in LÖVE. Keyboard interaction, visual quality, audible playback, installed VS Code menus, and preview navigation were **not** observed. `tua check` alone is never counted as runtime proof.

## How to run

From this directory, after building the repository's CLI:

```sh
/Users/kyonru/Documents/projects/Experiments/tlua/target/debug/tua check
/Users/kyonru/Documents/projects/Experiments/tlua/target/debug/tua build
/Applications/love.app/Contents/MacOS/love build
/Users/kyonru/Documents/projects/Experiments/tlua/target/debug/tua test
```

The program's `TUA_ALCHEMY_SMOKE=1` mode asserts recipe, event, store, ECS, pool, audio state, vector, camera, and assistant behavior, then exits. Run it with `python3 run_love_smoke.py .` (requires a graphical LÖVE host). `python3 run_hot_reload_smoke.py` copies `build/` to a temporary directory, starts LÖVE, waits for the initial shader load signal, edits an included GLSL file, and asserts replacement. It does not modify source or build output. Generated `build/` is ignored and should not be committed.

## Verification log

| Command / environment | Actual outcome |
| --- | --- |
| `target/debug/tua fmt --check` | exit 0; checked 4 files (formatter finding below remains because formatted output is the problem). |
| `target/debug/tua lint` | exit 0; linted 4 files. |
| `target/debug/tua check` | exit 0; checked 4 files; one TL5007 pool warning, discussed below. |
| `target/debug/tua build` | exit 0; built 9 files; same warning. Generated `main.lua` injects helper `require`s only for referenced modules, including AudioDesign, Rot, BehaviorTree, ShaderScan, ECS, store, state machine, sprite/anim8, input, camera, tween, vector, signal, pool, and audio. Assets copied under `build/assets/`. |
| `target/debug/tua test` | exit 0; **3 successes, 0 failures/errors/pending**, covering unordered recipe, base-class virtual dispatch, imported generic jar, and missing recipe. |
| `luac -p build/main.lua build/model.lua build/recipes.lua` | exit 0; generated Lua syntax valid. |
| `python3 run_love_smoke.py .` with `/Applications/love.app/Contents/MacOS/love` | exit 0; `SMOKE: shader attempted true`, `SMOKE: recipe, signal, store, ECS, pool passed`, `SMOKE: pool release passed`; empty stderr. Real LÖVE execution, but no manual input or audio perception. |
| `python3 run_hot_reload_smoke.py` | exit 0; `HOT: initial shader loaded`, `[ShaderScan] reloaded pulse (assets/pulse.glsl)`, `HOT: included shader reloaded`; empty stderr. The first unsynchronized attempt edited before initial load and timed out; it was a test-harness error, fixed by waiting for a load signal. |
| `node lsp_probe.js`, `node lsp_probe.js --incomplete`, `node lsp_simple_probe.js`, `node lsp_edit_probe.js` | Actual Tua LSP protocol requests; details below. No installed VS Code widget measurement. |

Warm CLI measurements, three sequential process invocations each on this 4-source-file project (milliseconds): `check` 1891.8/1873.0/1876.5; incremental `build` 2460.4/2466.8/2421.7; `fmt --check` 47.9/45.7/44.7. These include process startup and dependency analysis; concurrent audit agents may have contended for resources. No baseline or latency target was established. LSP completion on a fresh server took 755–801 ms for the cross-file probe, warm repeat 0.44–0.45 ms server round trip. These are protocol timings, **not** key-to-menu timings.

## Feature coverage

“Used” here means checked/built plus the named runtime, test, or protocol observation. “Inspected” means no observed user workflow. File references are relative to this directory.

| Coverage row | Status and evidence |
| --- | --- |
| Typed locals, records, arrays, unions, optional/nil, aliases | **Used:** `src/main.tua:5-9,119,144`, `src/recipes.tua:4-5,31`; check/build/test. Negative `repros/typing_contract/` gives TL1003 at `count`: expected number, found string. |
| Functions, callbacks, multi-returns, closures, narrowing | **Used:** `src/main.tua:21,73,144,151-156,164-181`, `src/recipes.tua:31-39`; check/build and LÖVE smoke. `if potion then` narrows; `local _, playback` receives two audio values. |
| Classes, inheritance, virtual dispatch, instance aliases | **Used:** `src/model.tua:6-61`, `tests/recipes_spec.tua:6-15`; Busted asserts a base-typed array dispatches Potion override. |
| Generic alias, function, class, imported specialization | **Used:** `src/recipes.tua:5,16-29`, `src/model.tua:39-61`, `tests/recipes_spec.tua:18-23`; check and runtime Busted. LSP completed-member hover gives `Jar<Potion>`. |
| Protocol conformance and bound across files | **Used:** `src/model.tua:2-8`, `src/recipes.tua:1,16,20,24`; positive check/runtime. Negative `repros/typing_contract/` exit 1 with TL1008 missing `name` on `Missing`. |
| Tua imports, plain Lua interop, project exports | **Used:** `src/main.tua:1-3`, `src/legacy.lua`, `src/recipes.tua:50`; check/build and LÖVE uses Lua `Legacy.clamp`. |
| LOVE callbacks, object methods, asset constructors | **Used:** `src/main.tua:183-284`; LÖVE smoke loaded font, sprite image, sound, shader, and entered update/draw. |
| Sprite sheet and animation | **Used with limit:** `src/main.tua:47-53,226,268`; LÖVE draw/update ran and no error. Exact frame order/timing and preview were not visually inspected. |
| Vec2 and game math | **Used:** `src/main.tua:17-18,209`; smoke asserts vector subtraction length 20. |
| Signals | **Used:** `src/main.tua:105,144-157,170`; brew event handler updates state and spawns sparks; smoke asserts consequences. |
| State stores | **Used:** `src/main.tua:20-28,147-151`; smoke asserts score 10; watcher starts tween, though its visual motion was not measured. |
| State machines | **Used:** `src/main.tua:30-37,164-181`; smoke asserts `showing` and `can("brew") == false` after transition. |
| Typed input / Baton | **Used with limit:** `src/main.tua:39-45,216-222`; LÖVE update calls binding reader. No synthetic key press or manual keyboard test, so response to a physical key remains unverified. |
| Tweening | **Used with limit:** `src/main.tua:21-28,225,281`; store watcher schedules a tween, update runs; final interpolated value not asserted. |
| Camera | **Used:** `src/main.tua:118,157,229-230,244-247,254`; smoke asserts toScreen/toWorld round trip after follow/update; draw callback ran. |
| Rot procedural helpers | **Used:** `src/main.tua:13-16,136,199-200`; seeded map/path/dice/color execute in LÖVE; map/path quality not assessed. |
| Behavior trees | **Used:** `src/main.tua:87-103,153,223,242`; `reactiveSelector` tick sets hint, smoke asserts text; reset called on brew. Only one actor, so independent actor state not exercised. |
| ECS | **Used:** `src/main.tua:55-86,131-141`; smoke checks 12 active pooled particles after event and zero after system updates. |
| Object pools | **Used:** `src/main.tua:60-72,73-81,131-141`; same LÖVE count assertions; TL5007 warning does not recognize release through ECS alias. Reuse after release was not measured. |
| Audio cues, scenes, AudioDesign | **Used with limit:** `src/main.tua:106-116,154-156,188,208`; LÖVE created generated tone and audio graph; `audio:snapshot().music.state == "lab"` asserted; no audible output or exact voice/routing measurement. |
| ShaderScan, GLSL includes, uniforms, hot reload | **Used:** `src/main.tua:117,190,228,232-239,262-266`, `src/assets/{pulse,lib/pulse}.glsl`; LÖVE compiled shader, sent `time`, then isolated include edit replaced shader. Paired path failure documented below. |
| Asset metadata and source navigation | **Attempted/inspected:** image/font/audio/GLSL asset paths in `src/assets/` checked/built/loaded; LSP source navigation to assets or metadata was not requested. Shader source vs output path defect reproduced. |
| `tua fmt`, `lint`, `check`, `build`, generated Lua | **Used:** commands and output above; emitted Lua parsed by `luac`, and built game ran under LÖVE. Formatter issue reproduced separately. |
| `tua test` / Busted | **Used:** `tests/recipes_spec.tua:5-29`, 3 passing tests on actual generated model/recipes. |
| Completion, hover, signature, definition during edits | **Used at protocol layer:** `lsp_probe.js`/`LSP*.json`; completed `jar:label()` offers `[label,get]`, hovers `Jar<Potion>`, signature for imported `Recipes.mix`, definition to `src/recipes.tua:31`. Incomplete `jar:` loses member result; same-file simple control succeeds. |
| Feature previews and cross-view navigation | **Inspected only:** no installed VS Code host opened this fixture; cannot claim preview appearance, navigation, or accessibility. |
| Save/edit/recovery, dependency invalidation | **Used at protocol layer:** `lsp_edit_probe.js` unsaved `didChange` in model adds `Jar.extra()`; completion evolves `[label,get]` → `[label,get,extra]` → `[label,get]` on revert. File rename/delete and cross-project cache isolation not tested. |
| Performance: cold/warm suggestion, check/build, format | **Used at protocol/CLI layer:** measurements above; visible typing-to-menu and format-on-save not measured. |

## Findings and development friction

### ALC-01 · Shader include path differs between project source and built game (confirmed defect, Medium)

The first API path tried was `docs/shaders.md:35-39` (“project-root-relative include paths”) and the bundled `examples/features/src/shaders.tua:3-7`, which explicitly calls `load_shader(..., "src/assets/wave.glsl")`. I used `#include "src/assets/lib/pulse.glsl"` and `load_shader(..., "src/assets/pulse.glsl")`. The checker and build accepted both, but `build/assets/` contains assets without the `src/` prefix and LÖVE's runtime `love.filesystem.read` cannot find them. Minimal paired fixtures: `repros/shader_source_path/` and `repros/shader_build_path/`. Run `tua check && tua build` in each directory, then `python3 ../../run_love_smoke.py .`. The source-path fixture prints `source-path load false ... cannot read shader include "src/assets/pulse.glsl" ... Does not exist.`; the build-path control prints `build-path load true nil`. Both checks pass. The main game uses `assets/pulse.glsl` as a workaround. Likely boundary: project asset path resolution/copying and runtime `love.filesystem.read` in `build/tuashaderscan.lua:28-33,106-112` use different roots; docs/example present the former as runtime-safe. Fix direction: specify one runtime asset path contract, translate source-relative paths during lowering if supported, and test build-plus-LÖVE load from `src/assets/`. At minimum correct the example/docs and diagnose non-runtime paths. Parent agent independently rechecks this pair.

### ALC-02 · Imported generic member completion disappears at unfinished colon (confirmed protocol defect, Medium)

`node lsp_probe.js --incomplete` sends `src/editor_probe.tua` as an unsaved buffer. At zero-based 6:6, the line is exactly `  jar:` where `jar` is the unannotated result of imported `Recipes.seal(potion)` inside `if potion then`. Completion returns `[]`, hover is null, though imported `Recipes.mix` signature and definition still work. `node lsp_probe.js` sends `  jar:label()` at the same cursor and returns completion `[label,get]`, hover `Jar<Potion>`, no diagnostics. The incomplete source produces two TL0001 parser diagnostics. `node lsp_simple_probe.js` sends a same-file non-generic `object:` and completion **does** return `label` despite the same parser error. The problem is therefore narrower than universal colon recovery; likely imported generic type/provenance or recovery interaction, not established by source inspection. Saved exact responses: `LSP-incomplete.json`, `LSP.json`. Fix direction: keep receiver type available to member completion through incomplete expression recovery; regression should compare same-file and cross-file specialized generic receivers, complete and incomplete, at exact cursor positions. Installed VS Code widget behavior remains unverified.

### ALC-03 · Audio snapshot music shape is `any` (confirmed type capability gap, Low)

My first smoke assertion used `audio:snapshot().musicState == "lab"`; `tua check` accepted it, while LÖVE failed `main.lua:245 assertion failed`. Inspecting `crates/tua-core/src/audio.rs:448-459` shows `snapshot_type()` declares nested `music` as `Type::Any`; runtime shape is `audio:snapshot().music.state`. Corrected source at `src/main.tua:208` passes LÖVE. This was my trial-code typo, not an audio runtime failure. The typing gap means similar snapshot property mistakes are silent; define the actual nested record from the runtime and test hover/completion and negative checks for invalid fields. Preserve the exact incorrect assertion above as a regression example.

### ALC-04 · Pool warning misses release through ECS component alias (confirmed diagnostic limitation, Low)

`src/main.tua:133` acquires `particle`, attaches it as `SparkData` at line 139, and the ECS system callback at lines 73–80 releases the same table as `spark`. `tua check` emits TL5007: “no later same-file `sparks:release(particle)` or `sparks:clear()` is visible.” Actual LÖVE smoke checks `activeCount()` 12 after spawn and 0 after 0.75 seconds. The warning's own wording limits it to visible same-file release; it is a conservative warning, not a false claim of a leak. It is noisy for common ECS ownership transfer. Source `crates/tua-core/src/object_pool.rs:463` constructs this warning. Fix direction: add explicit suppression/ownership transfer or narrow warning to provably held locals; do not attempt arbitrary interprocedural alias tracking just for this case. Add ECS handoff fixture.

### ALC-05 · Formatter indents top-level return after generic type alias (confirmed formatter defect, Low)

`repros/format_generic/src/main.tua` contains an exported generic class and `export type BoxInstance<T> = Box<T>` followed by a top-level `return { Box = Box }`. Run `tua check` (passes), `tua fmt` (produces `  return { Box = Box }`), then `tua fmt --check` (passes). The trial's `src/model.tua:61` and `src/recipes.tua:20-29` show related indentation around generic syntax. Lua ignores the whitespace, so runtime behavior is unaffected; readability and predictable format-on-save suffer. Likely formatter nesting state around generic/class declaration, root cause not yet traced. Add golden source formatting regression for this exact fixture.

### ALC-06 · Generic class bound export workflow is unclear (design/documentation question)

I first tried `export type JarInstance<T: Named> = Jar<T>` after a `final class Jar<T: Named>`; the parser rejects a bound parameter in that alias, so I changed to an unbounded `Jar<T>` and constrained the imported `seal<T: Named>` function instead. This trial does not establish that bounded exported class aliases are part of Tua's documented grammar. Investigate whether class bounds propagate through instance aliases and document the idiomatic form. Do not classify this as a compiler bug without a contract.

## Improvements suggested by the trial

1. Resolve the ShaderScan path contract and bundled example first: a checked, built game can still fail at runtime with the documented-looking path.
2. Preserve imported specialized receiver information while the user is typing `:`. This is the primary editor friction observed in this trial.
3. Define nested audio snapshot records, including `music.state`, so property typos are caught at check time.
4. Make ownership-transfer patterns to ECS less noisy for TL5007, while keeping the warning honest about uncertainty.
5. Keep formatter output stable around generic aliases and top-level returns.

**Limits:** I did not run an installed VS Code host, simulate actual key input, listen to audio, visually inspect sprite animation or preview webviews, measure retained memory, or compare a previous Tua version. The protocol numbers do not imply visible suggestion latency. The game and minimal reproductions can be rerun without the private Zombie project.
