# Tua tutorials: implementation plan

Status: in progress; reorganized around **one evolving game with runnable chapter
checkpoints**, as requested. M0–M4 delivered useful foundations, but the standalone
feature-by-feature format is superseded by the chapter plan below. The existing
[first playable dialogue](../tutorials/first-dialogue.md) remains a short entry point.
This plan does not authorize new runtime APIs or release claims.

## Outcome

Help someone build a small playable arena game, adding a meaningful capability in
each chapter. The same character, enemies, controls and progress continue throughout.
Tutorials teach how APIs work together: who owns state, what updates first, what
pauses, and what survives a restart. A chapter needs a playable outcome rather than
merely demonstrating an API. Feature pages own small focused examples, API defaults,
compatibility and detailed lifecycle reference material.

Each tutorial must answer: what will I build, what do I need, how do I run it,
what should happen, and what can I add next?

## One evolving project

The flagship tutorial is a small arena game. Begin with a movable Kenney character
and a playable/pause loop; add a weapon and enemy waves together, then deepen that
same game. Keep the art, layout and controls consistent. No grid backgrounds or
generic dashboard decoration. Use Kenney assets when artwork is needed, with bundled
licenses and provenance; primitives are fine for bullets, hit indicators and HUD.

The working first result should remain small. Advanced chapters are optional stops
on the same path, not prerequisites for firing the first shot. Do not introduce a
framework, project generator or all future systems in chapter one.

### Runnable chapter checkpoints

- Own the series under `examples/tutorials/arena-game/checkpoints/` and its guide
  under `docs/tutorials/arena-game/`. A series README lists chapters, prerequisites,
  controls and exact commands. Each checkpoint has its own `tua.toml`, `src/`,
  README, `.gitignore` and relevant tests.
- Use numbered, stable folders such as `01-playable-loop` and `02-combat-waves`.
  Every folder runs independently. No imports from a later checkpoint, another
  tutorial or the zombie game; the reader can copy one folder and work from it.
- Author a chapter by extending the preceding checkpoint. Explain the actual file
  changes, preserve recognizable names and show the same game growing. Do not replace
  the architecture wholesale each chapter or expose later concepts in early code.
- A small amount of source duplication is intentional so each checkpoint is a real
  learning boundary. Do not maintain an additional mutable `latest` copy. The index
  links to the highest completed checkpoint.
- Bundle only the assets required at each checkpoint. Record hashes so repeated
  assets can be verified. Avoid runtime downloads or special asset-generation steps.
- When correcting a copied defect, inspect every later checkpoint containing it and
  run their affected regressions. Keep finished checkpoints runnable after compiler
  updates; they are maintained examples, not frozen binaries.
- Each chapter explains its starting point, changed files, new behavior and a short
  experiment. Test the cumulative project, not only the new helper in isolation.

## Chapter delivery plan (authoritative)

These chapters replace the old one-API-topic delivery order. The M sections below
retain detailed acceptance requirements and historical evidence; their old completion
status does **not** establish that the integrated chapters are complete.

| Chapter / checkpoint | Playable outcome | APIs and requirements carried forward | Status |
| --- | --- | --- | --- |
| C1 — `01-playable-loop` | Move an animated character, start, pause, resume and restart the same run | M1 + M2: input, sprites, camera, state machine; focus and input ownership | Complete; see C1 qualification |
| C2 — `02-combat-waves` | Aim, fire projectiles, defeat enemies and start another bounded wave | M4 + M5: ECS, pool, tween, camera; swept hits, health, cadence, recoil and reduced motion | Complete; cumulative checkpoint and qualification |
| C3 — `03-enemy-decisions` | Enemies detect and approach the player, attack, and produce a game-over transition | M8: behavior trees + ECS; actor cancellation and cooldowns; reuse C1 flow | Complete; C3 qualification records selected-compiler typing limits |
| C4 — `04-sound` | Hear shots, hits and scene transitions; adjust music/effects or mute | M6: audio cues/buses; pause, concurrency and resource ownership | Complete; fake-backend and native Source qualification |
| C5 — `05-visual-feedback` | Add a scoped hit shader to the existing combat and show loading/failure states | M7: shader loading, initialization and optional preload; retain feedback controls | Pending |
| C6 — `06-conversation-and-language` | Meet a character between waves and switch the game's UI/dialogue locale | Existing dialogue starter + M9: dialogue, catalogs, layout; explicit modal/nonmodal ownership | Pending |
| C7 — `07-save-progress` | Leave and resume durable game progress and preferences across launches | M3: typed saves, migration and deliberate recovery; no saved ECS IDs or runtime handles | Pending |
| C8 — `08-profile-the-game` | Inspect and measure this game's bounded larger waves, then evaluate one justified optimization | M10: inspector/profiler, reproducible workload, unchanged outcomes | Pending |
| C9 — Qualify the whole path | Follow the chapters from an empty project through the final playable game | M11 plus cumulative checkpoint verification and migration/link audit | Pending |

C2 deliberately merges weapons and waves: a pool exhaustion rule affects when a shot
can exist; a swept collision selects a live ECS enemy; one accepted hit changes health;
feedback observes the result without deciding damage. Teach those connections rather
than an isolated dummy-target shooter followed by an unrelated enemy example.

For C1, combine existing movement and game-flow behavior into a coherent small host;
do not simply paste two entry points together. For C2, agree on one world-space aim
conversion and simulation clock so camera feedback cannot alter hit results. Carry
pause, focus and reset policy into every later chapter. For C7, define a checkpoint
boundary (for example, between waves) before choosing what to persist; saving a live
combat snapshot is not an implicit requirement.

### Immediate work and migration

- [x] C1: create the first combined checkpoint from the qualified M1/M2 foundations,
      with native movement/pause/restart verification and a cumulative walkthrough.
- [x] C2: adapt the weapon prototype to live ECS enemies and the C1 player/camera.
      Keep its rate, pooling and swept-collision tests; add nearest-enemy selection,
      one-hit/one-damage, pause/reset and feedback-independent outcome regressions.
- [ ] Fold later M requirements into the same game in chapter order above; do not
      add new independent `scene-audio`, `first-shader` or `enemy-decisions` projects.
- [x] Publish a project landing page and chapter navigation when C1 is runnable.
      Until then, keep the current guides honest about being standalone foundations.
- [ ] Review each existing standalone guide after its replacement is qualified.
      Move short API-only material into its owning reference page; use a concise
      redirect/link for superseded guide URLs where supported by the docs stack.
- [ ] Preserve existing example paths and tests until their consumers are audited.
      `pause-input`, `love-save` and the dialogue starter are existing reference
      examples, not automatically deletable duplicates.
- [ ] Remove redundant tutorial-only projects only after documenting their replacement,
      updating links and preserving any unique regression coverage. Keep migration
      cleanup separate from gameplay/chapter changes.

The original M5 prototype checked cadence at 30/60/120/144 Hz, bounded reuse, expiration,
swept target crossing, reset and disposal against emitted Lua. It served as implementation
input for C2. The integrated checkpoint is now separately qualified below.

## Organization and boundaries

- Publish completed chapters in the **Tutorials** navigation section. Keep this plan
  out of public navigation. Put a project outcome before the chapter list.
- Keep short first-use examples on reference pages; cross-link to the game chapter
  for composition. A useful existing standalone entry point may remain available.
- Use existing Tua features and inspect current docs/source before choosing APIs.
  Report demonstrated gaps with minimal reproductions; keep upstream fixes separate.
- Keep source in `.tua`; generated Lua remains ignored build output. Prefer readable,
  concrete code over a reusable tutorial framework.
- The single-game series is the current scope. Additional conversation-only or visual
  scene series require a distinct learning outcome; do not create them merely to
  accommodate another API.

## Teaching contract

Each guide follows the same small structure, with length determined by its subject:

1. Show the finished result and give exact run commands and controls.
2. State compiler/runtime requirements and what the reader can leave at defaults.
3. Build the smallest useful version in three to five coherent steps. Identify
   filenames and whether each snippet is complete or an edit to an earlier file.
4. Explain ownership and lifecycle at the point where they affect correct use.
5. Offer two or three experiments with an observable expected result.
6. Link optional next features and likely troubleshooting cases to their owners.

Keep each chapter's runnable checkpoint authoritative. Validate the cumulative edits
from the previous checkpoint, not just explanatory excerpts. The selected chapter
checkpoint structure intentionally permits independently runnable stages. Keep test
scaffolding out of the main learning path and explain it in the checkpoint README.

Clearly separate engine defaults from example policy: starting state, input maps,
pausing, volume, save location, pool limits and timing should not look like implicit
Tua behavior. Avoid performance promises, arbitrary target frame rates and examples
that require every helper to demonstrate one feature.

## Original milestone map (requirements and historical evidence)

| Milestone | Tutorial / proposed slug | Why this order |
| --- | --- | --- |
| M0 | Establish conventions | Audit APIs and example ownership before adding guides |
| M1 | Your first playable character / `playable-character` | Provides input, drawing and update fundamentals |
| M2 | A pause menu and game flow / `game-flow` | Introduces input and update ownership early |
| M3 | Save and resume a run / `save-and-resume` | Teaches persistence before consequential game systems |
| M4 | A wave of enemies / `enemy-wave` | Introduces entity identity and lifecycle |
| M5 | A weapon that feels responsive / `responsive-weapon` | Builds on lifecycle concepts without requiring M4 source |
| M6 | Bring a scene to life with audio / `scene-audio` | Adds presentation and resource ownership |
| M7 | Your first shader effect / `first-shader` | Adds GPU resources and initialization boundaries |
| M8 | Enemies that make decisions / `enemy-decisions` | Combines decision logic with the established lifecycle model |
| M9 | Translate a small game / `translate-a-game` | Applies localization to a small UI and conversation |
| M10 | Find and fix a slow frame / `slow-frame` | Uses completed workload patterns to teach evidence-based tuning |
| M11 | Qualify the complete learning path | Check discoverability, consistency and executable instructions |

This table records the original topic breakdown, not the current shipping order.
Use C1–C9 above for new work. Retain the M acceptance requirements when integrating
their behavior into chapters.

## M0 — Establish conventions

- [x] Audit the current feature docs, examples and selected compiler for each guide.
- [x] Record each tutorial's owning example, required assets and verification command.
- [x] Use the dialogue starter as a reference for scope, not a mandatory controller
      architecture or visual template.
- [x] Define common README headings: run, controls, expected behavior, try a change,
      verification and next steps.
- [x] Define where native replay evidence and compiler/runtime provenance are kept;
      retain only useful screenshots in docs, not entire capture frame sequences.

Acceptance: every proposed guide has a confirmed starting point; uncertainties are
explicit follow-ups, not invented APIs embedded in prose.

## M1 — Your first playable character

References: [input](../input.md), [sprites](../sprites.md), [camera](../camera.md).

- [x] Move a visible character using typed input and delta time.
- [x] Add idle/walk animation with a small bundled sprite sheet.
- [x] Add a following camera and explain world-space versus screen-space drawing.
- [x] Explain diagonal movement, focus loss and animation transition policy.
- [x] Offer movement-speed and camera-follow experiments; keep combat optional.

Acceptance: movement behaves consistently over different update step sizes, stopping
input stops movement, animations change correctly, and a screen-space HUD remains
fixed while the camera moves. Capture a native movement replay.

## M2 — A pause menu and game flow

References: [state machines](../state-machines.md), [input](../input.md).

- [x] Build title → playing → paused → playing transitions and a restart action.
- [x] Make one state own each input action; distinguish UI and gameplay updates.
- [x] Keep the paused world visible while preventing simulation advancement.
- [x] Explain enter/exit cleanup and restarting a session without duplicating listeners.
- [x] Offer a game-over state as an extension rather than initial setup.

Acceptance: pause freezes gameplay but leaves menu input working; one press causes
one transition; repeated pause/resume/restart does not duplicate entities or callbacks.

## M3 — Save and resume a run

Reference: [typed save contracts](../save-contracts.md).

- [x] Start with a small progress record and explicit save/load controls.
- [x] Define validation, defaults and user-visible outcomes for absent or invalid saves.
- [x] Introduce a historical schema and migration only after basic save/resume works.
- [x] Explain supported recovery guarantees and failure boundaries using the existing
      implementation; do not describe an ordinary file write as crash-safe.
- [x] Save serializable progress, not entity IDs, runtime objects or resource handles.

Acceptance: a second launch restores progress; missing, corrupt, old and unsupported
future saves follow the documented policy. Fixtures use an isolated save identity
and never overwrite real user/game data. Keep inventory transaction recovery optional.

## M4 — A wave of enemies

References: [ECS](../ecs.md), [object pooling](../object-pooling.md).

- [x] Define position, movement and health components and create a small wave.
- [x] Update through queries; remove defeated entities through ECS lifecycle APIs.
- [x] Explain component replacement, live identity and supported structural mutations.
- [x] Add pooling only for a suitable transient object, such as a defeat effect,
      after demonstrating correct lifecycle without it.
- [x] Keep wave scheduling and spawn limits explicit host policy.

Acceptance: defeated enemies stop updating, repeated resets leave no stale entities,
and the optional pooled effect resets all retained state. ECS stays authoritative;
pooling is not presented as a second enemy registry or an automatic speedup.

## M5 — A weapon that feels responsive

References: [pooling](../object-pooling.md), [tweening](../tweening.md),
[camera](../camera.md).

- [x] Fire a projectile with explicit cadence, movement, lifetime and simple hit handling.
- [x] Add bounded projectile reuse and a clear exhaustion policy.
- [x] Add visual recoil, an impact effect and restrained camera feedback separately.
- [x] Explain which effects are cosmetic and which affect simulation.
- [x] Include reduced-motion/feedback controls and a clear reset path.

Acceptance: cadence is stable across update rates, expired projectiles are reclaimed,
reused projectiles contain no stale hit data, and one hit produces one damage result.
Use an appropriate swept test if the demonstrated speed can cross a target in one step.
Feedback settings must not change damage or hit outcomes.

## M6 — Bring a scene to life with audio

Reference: [audio](../audio.md).

- [x] Play one effect through a cue and one music track through its intended bus.
- [x] Add separate music/effects volume controls and demonstrate a scene transition.
- [x] Explain source creation, reuse, concurrency limits and shutdown ownership.
- [x] Keep generated or licensed audio small and provide a mute control.
- [x] Distinguish host pause policy from volume and transport state.

Acceptance: controls affect the intended buses, transitions do not accumulate music
sources, and restart/disposal releases owned resources. Verify adapter logic with a
fake backend and playback in native LÖVE; do not call silent video an audio check.

## M7 — Your first shader effect

Reference: [shaders](../shaders.md).

- [ ] Start with a hit flash on one object and split shader source into supported files.
- [ ] Show typed loading, uniforms and the boundary of the effect's draw scope.
- [ ] Initialize a candidate using the existing `on_compiled` hook.
- [ ] Add optional `preload` progress while `update` processes queued entries.
- [ ] Explain missing/failed shader handling and first-draw warming separately.

Acceptance: the effect does not leak onto subsequent draws; initialization and
failure paths match the loader contract; preload feedback settles correctly.
State explicitly that queued preloading does not make an individual GPU compilation
nonblocking. Water is a later experiment, not a second required tutorial project.

## M8 — Enemies that make decisions

References: [behavior trees](../behavior-trees.md), [ECS](../ecs.md).

- [x] Implement patrol, detect, chase and attack with a small typed actor context.
- [x] Explain condition evaluation, running/success/failure results and cancellation.
- [x] Integrate the runner with entity lifecycle after the standalone behavior works.
- [x] Keep any confirmed dynamic typing boundary inside a small documented adapter.
- [x] Offer detection radius and cooldown experiments without adding pathfinding.

Acceptance: scripted target positions yield deterministic transitions, attack
cooldowns hold, and destroyed/reset actors cannot continue attacking. Verify wrong
context fields are rejected where current tooling promises that support.

## M9 — Translate a small game

References: [localization](../localization.md), [dialogue](../dialogue.md).

- [ ] Begin with a small menu and one short conversation in the source language.
- [ ] Add typed catalog keys, interpolation and a second locale.
- [ ] Switch language at runtime without resetting gameplay or replaying dialogue effects.
- [ ] Demonstrate missing-key/fallback behavior and a longer translated label.
- [ ] Explain fonts, wrapping and shaping responsibilities without claiming full
      international text support from catalog lookup alone.

Acceptance: invalid literal keys fail where supported, interpolation preserves data,
locale refresh is side-effect free, and long text stays usable. Document the tested
scripts/fonts and fallback behavior; keep speech localization optional.

## M10 — Find and fix a slow frame

Reference: [debugging and profiling](../debugging.md).

- [ ] Build a bounded, adjustable workload with fixed seed and reproducible input.
- [ ] Demonstrate the inspector for state and profiler for measured hot paths.
- [ ] Confirm one bottleneck, apply one relevant change, and compare matching outcomes.
- [ ] Separate profiled diagnosis from uninstrumented CPU timing and allocation probes.
- [ ] Record compiler/runtime revisions, JIT status, hardware, workload, warm-up,
      repetitions, GC policy, median/p95 and variation.

Acceptance: the diagnosed cause is supported by measurements and the optimization
preserves outcomes. No absolute timing assertion, universal enemy-count limit or
simulation-ms-to-rendered-FPS claim. Distinguish update/draw from complete frame time,
GC-stopped allocation from retained heap, and sampled GC activity from exact duration.
If an optimization does not help on a tested system, report that result honestly.

## M11 — Qualify the learning path

- [ ] Check every guide from a clean example directory using its exact commands.
- [ ] Validate cumulative snippets and compare the final state with the owning example.
- [ ] Confirm all guide links, feature anchors, prerequisites and navigation labels.
- [ ] Review complexity: remove optional setup from the first working result.
- [ ] Check that defaults and lifecycle responsibilities are explicit and consistent.
- [ ] Keep the tutorial index focused on goals and next steps, not API inventories.
- [ ] Label agent-led walkthroughs as simulated checks, not human usability studies.

## Shared verification and delivery gate

For each tutorial:

- Run `tua fmt` and `tua fmt --check` in the owning example; format/check authored
  Lua harnesses explicitly. Select the exact compiler used for qualification.
- Run `tua check`, build, and focused tests against emitted Lua. Exercise meaningful
  behavior and failure boundaries; do not add assertions that merely restate constants.
- Use supported interpreters available locally. Run native LÖVE when drawing,
  input, audio, GPU behavior or resource lifecycle matters; report unavailable checks.
- Capture and inspect the relevant visual states. Add screenshots or short video
  only where they explain the tutorial outcome; record runtime/compiler provenance.
- Run `make docs-build` and `make diff-check`; check links and navigation changes.
- Verify assets have licenses/provenance and generated output is not committed.
- Record completion, example location, exact checks, evidence and remaining limits
  in this plan. Do not mark a tutorial complete from prose or a successful build alone.

Deliver each tutorial as a coherent unsigned commit, or a small example/docs pair
when review benefits from separation. Keep compiler fixes, unrelated formatting and
other contributors' work separate. Do not push or publish without authorization.

## Completion ledger

| Milestone | Status | Example / commits / evidence |
| --- | --- | --- |
| Existing dialogue starter | Complete | `examples/dialogue/first-conversation`; `b1d4d112`, `edb2c04c`, `52647cee` |
| M0 | Complete | API/source audit below; no runtime changes required |
| M1 | Complete | `examples/tutorials/playable-character`; `8a11c484`; [qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/playable-character.md) |
| M2 | Complete | `examples/pause-input`; `257701a2`; [qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/game-flow.md) |
| M3 | Complete | `examples/love-save`; `46a9df23`; [qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/save-and-resume.md) |
| M4 | Complete | `examples/tutorials/enemy-wave`; `936b615a`; [qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/enemy-wave.md) |
| M5 | Complete in C2 | `02-combat-waves`; [qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/arena-game-02.md) |
| M6 | Complete in C4 | `04-sound`; [qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/arena-game-04.md) |
| M7 | Pending | |
| M8 | Complete in C3 | `03-enemy-decisions`; [qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/arena-game-03.md) |
| M9 | Pending | |
| M10 | Pending | |
| M11 | Pending | |

## Original M0 audit and implementation baseline

The owning-example decisions below are historical; chapter ownership above supersedes
new standalone project proposals.

Audited against compiler 0.11.0, embedded revision
`e0033fc65d3b78f9be8aba3e427aa9a239782354`, release aarch64-apple-darwin,
SHA-256 `3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
This identifies the selected development binary, not a feature promise for every
0.11.0 distribution. Re-record provenance when selecting a different compiler.

The focused pages linked in each milestone and corresponding `examples/features`
source confirm the relevant entry points. This is an API/example audit, not a claim
that all ten future compositions have passed runtime qualification.

| Guide | Owning example decision | Existing source to consult | Asset / API boundary |
| --- | --- | --- | --- |
| M1 | New `examples/tutorials/playable-character` | `features/src/input.tua`, `sprites.tua`, `camera.tua` | Kenney CC0 character atlas, unchanged PNG; `input`, `spriteSheet`, `Camera.new` |
| M2 | Extend `examples/pause-input` | `features/src/state_machine.tua`, existing pause scenario | Primitives; add explicit title/state-machine flow without weakening held-input tests |
| M3 | Extend `examples/love-save` | `features/src/save_contracts.tua` | No assets; versioned `Save.contract`, historical schemas and explicit recovery |
| M4 | New `examples/tutorials/enemy-wave` | `features/src/ecs.tua`, `object_pool.tua` | Primitives; `ECS`, query/lifecycle APIs, optional `objectPool` effect |
| M5 | New `examples/tutorials/responsive-weapon` | Pooling/tween/camera feature sources | Primitives; bounded reuse and cosmetic feedback remain separate |
| M6 | New `examples/tutorials/scene-audio` | `examples/audio-system`, `features/src/audio.tua` | Small generated/licensed audio; `audioSystem` sources and bus ownership |
| M7 | New `examples/tutorials/first-shader` | `features/src/shaders.tua`, `shader_preload.tua` | Authored GLSL; existing `ShaderScan` setup/preload callbacks |
| M8 | New `examples/tutorials/enemy-decisions` | `features/src/behavior_tree.tua`, `behavior_tree_ecs.tua` | Primitives; tree definition versus per-actor runner, context checking |
| M9 | New `examples/tutorials/translate-a-game` | `features/src/localization.tua`, `examples/dialogue/localization` | Plain catalogs and built-in font for selected Latin locales; opt-in i18n |
| M10 | New `examples/tutorials/slow-frame` | Existing profiler workflow in `docs/debugging.md` | Primitives; workload bounds, no second profiler or timing thresholds |

For each owning example, run `tua fmt --check`, explicit formatting checks for
its Lua harness, `tua check`, and `tua build` inside that directory. Use
`scripts/love_replay.py --project <owner> --scenario <owner>/tests/replay.lua`
for new LÖVE scenarios; retain the existing pause scenario-module command when
extending that example. Supply the exact compiler, LÖVE executable, frame count
and a new output directory in each README's completed recipe. Specialized save,
audio and performance checks are specified in their milestone and must be added
before those guides can be marked complete.

Evidence convention: native captures live outside tracked source by default;
store a compact qualification record under `benchmarks/tutorials/<slug>.md` with
commands, compiler/runtime identity, outcomes and limitations. Retain selected
screenshots under `docs/assets/tutorials/<slug>/` only when used in the guide,
with provenance. Do not commit all replay frames. Example READMEs use Run,
Controls, Expected behavior, Try a change, Verification and Next steps headings.

M1-specific audit findings: Baton normalizes diagonal pairs before applying its
deadzone; repeated `sprite:play` on the active animation preserves its progress;
`Camera:draw` restores the world transform before HUD drawing. Focus handling is
host policy. Per the user’s asset preference, M1 uses the unchanged Kenney Platformer Characters
adventurer atlas, with bundled CC0 license and archive/file hashes.

## M1 completion

Published the playable-character walkthrough and Tutorials index. The independently
runnable example uses Kenney CC0 art, typed normalized input, idle/walk animation,
camera following, explicit focus policy and resource cleanup. It has no grid
background or decorative panels. See the linked qualification record for native,
headless, formatting, emission and editor-protocol checks and their limitations.

Next milestone at the time of M1 completion was M2; see its completion below.

## M2 completion

Extended the existing pause/input example with typed title/playing/paused flow and
published the game-flow tutorial. Transitions consume their frame, input stays
polled across inactive states, gameplay/UI clocks remain separate, and focus loss
requires explicit resume. Reset replaces tween groups and retains input edge state.
The legacy replay and new lifecycle replay both passed, including 100 transition
cycles. The qualification record includes exact commands and platform limitations.

Next milestone at the time of M2 completion was M3; see its completion below.

## M3 completion

Published save/resume guidance from explicit checkpoints through autosave, typed
historical migration and deliberate recovery/reset policy. The existing example
now separates UI state from saved progress and confirms deletion. Two isolated
LÖVE processes verified persistence plus missing/corrupt/old/future data paths,
write retry and reset failures. Native UI replay and documentation checks passed.
The qualification record retains a selected-compiler missing-field diagnostic
probe for rechecking against a rebuilt current compiler; no new runtime API was needed.

M4 now includes a standalone Kenney enemy wave, live component readers, deterministic
click targeting, reset/disposal checks and optional bounded rings. Native replay
checks the actual callbacks and drawing; no runtime or compiler changes were needed.

## C1 completion

Published the cumulative `01-playable-loop` checkpoint and project landing page.
Input, movement, sprites, camera and state-machine flow now share one session
boundary. Emitted-Lua tests cover 100 cycles; native replay covers pause/focus,
animation/camera freeze, transition input, reset and screen-space HUD. See
[qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/arena-game-01.md).
Existing standalone examples remain available; their migration audit is pending.

## C2 completion

Integrated aimed projectiles and live ECS health with the C1 session. Player movement
and bullets share fixed ticks; feedback observes counters and never changes damage.
Rate, lifecycle and swept-collision tests passed; native replay verified feedback
parity, aimed hits, pause/focus, reset, wave clear and next-wave behavior. See
[qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/arena-game-02.md).

## C3 completion

Added typed shared behavior definitions with independent ECS runners, patrol/chase/
attack branches, fixed-tick cooldowns and explicit cancellation. Relative sweeps
account for moving enemies; surviving intents apply only after projectile damage.
Player health and game over extend the existing session. Native replay verifies
natural defeat/restart and feedback parity. The selected compiler rejects incorrect
known-field types but accepts unknown context field assignment; see the exact probe
in [qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/arena-game-03.md).

## C4 completion

Added cue/bus declarations, title/combat music scenes, independent gains and mute,
with typed testable host policy. Native Source clocks verify pause/resume; repeated
restarts retain one music voice and shutdown stops playback. Original generated WAV
fixtures are bundled with provenance; the replay MP4 remains silent. Enemy shadows
were added separately to the existing combat examples and carried into C4. See
[qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/tutorials/arena-game-04.md).

Next: C5, scoped hit shaders, initialization and loading/failure states.
