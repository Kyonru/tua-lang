# Tua tutorials: implementation plan

Status: planned. The [first playable dialogue](../tutorials/first-dialogue.md)
is implemented and provides the initial model for this series. This plan covers
10 additional tutorials; it does not authorize new runtime APIs or release claims.

## Outcome

Help someone build a small working game feature before introducing its optional
capabilities. Tutorials teach composition and ownership. Feature pages remain the
source of truth for APIs, defaults, compatibility and detailed lifecycle rules.

Each tutorial must answer: what will I build, what do I need, how do I run it,
what should happen, and what can I add next?

## Organization and boundaries

- Publish completed guides under `docs/tutorials/` in the existing **Tutorials**
  navigation section. Keep this implementation plan out of public navigation.
- Add a concise tutorial index when the second guide is ready. Group guides by
  reader goals; do not turn the index into a prerequisite checklist.
- Prefer existing focused examples when they already provide an approachable
  starting point. Otherwise use `examples/tutorials/<slug>/` with its own
  `tua.toml`, README, typed source and focused verification. Choose and record
  the owning example during each tutorial's API audit.
- Preserve the existing dialogue starter location. Do not move working examples
  merely to make their directories match new ones.
- All final examples run independently. Conceptual prerequisites are optional
  reading; no example imports source from another tutorial or the zombie game.
- Use existing Tua features first. Confirm exact APIs and supported behavior in
  the selected compiler before writing snippets. Report demonstrated gaps with
  minimal reproductions; keep upstream fixes separate from tutorial changes.
- Keep source in `.tua`; generated Lua remains build output. Prefer readable
  concrete code over building a framework for the tutorial series.
- Bundle small, appropriately licensed assets only where needed. Use primitives
  elsewhere; avoid mandatory downloads, accounts or network access at runtime.

## Teaching contract

Each guide follows the same small structure, with length determined by its subject:

1. Show the finished result and give exact run commands and controls.
2. State compiler/runtime requirements and what the reader can leave at defaults.
3. Build the smallest useful version in three to five coherent steps. Identify
   filenames and whether each snippet is complete or an edit to an earlier file.
4. Explain ownership and lifecycle at the point where they affect correct use.
5. Offer two or three experiments with an observable expected result.
6. Link optional next features and likely troubleshooting cases to their owners.

Keep the final runnable source authoritative. Validate cumulative code from the
steps; do not maintain several copied projects for each stage unless the guide
actually needs independently runnable variants. Keep test scaffolding out of the
main learning path and explain it in the example README.

Clearly separate engine defaults from example policy: starting state, input maps,
pausing, volume, save location, pool limits and timing should not look like implicit
Tua behavior. Avoid performance promises, arbitrary target frame rates and examples
that require every helper to demonstrate one feature.

## Delivery order

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

M1–M3 are the first delivery tranche. Later guides can ship independently after
confirming their own API/example baseline. This order is editorial, not a runtime
dependency graph.

## M0 — Establish conventions

- [ ] Audit the current feature docs, examples and selected compiler for each guide.
- [ ] Record each tutorial's owning example, required assets and verification command.
- [ ] Use the dialogue starter as a reference for scope, not a mandatory controller
      architecture or visual template.
- [ ] Define common README headings: run, controls, expected behavior, try a change,
      verification and next steps.
- [ ] Define where native replay evidence and compiler/runtime provenance are kept;
      retain only useful screenshots in docs, not entire capture frame sequences.

Acceptance: every proposed guide has a confirmed starting point; uncertainties are
explicit follow-ups, not invented APIs embedded in prose.

## M1 — Your first playable character

References: [input](../input.md), [sprites](../sprites.md), [camera](../camera.md).

- [ ] Move a visible character using typed input and delta time.
- [ ] Add idle/walk animation with a small bundled sprite sheet.
- [ ] Add a following camera and explain world-space versus screen-space drawing.
- [ ] Explain diagonal movement, focus loss and animation transition policy.
- [ ] Offer movement-speed and camera-follow experiments; keep combat optional.

Acceptance: movement behaves consistently over different update step sizes, stopping
input stops movement, animations change correctly, and a screen-space HUD remains
fixed while the camera moves. Capture a native movement replay.

## M2 — A pause menu and game flow

References: [state machines](../state-machines.md), [input](../input.md).

- [ ] Build title → playing → paused → playing transitions and a restart action.
- [ ] Make one state own each input action; distinguish UI and gameplay updates.
- [ ] Keep the paused world visible while preventing simulation advancement.
- [ ] Explain enter/exit cleanup and restarting a session without duplicating listeners.
- [ ] Offer a game-over state as an extension rather than initial setup.

Acceptance: pause freezes gameplay but leaves menu input working; one press causes
one transition; repeated pause/resume/restart does not duplicate entities or callbacks.

## M3 — Save and resume a run

Reference: [typed save contracts](../save-contracts.md).

- [ ] Start with a small progress record and explicit save/load controls.
- [ ] Define validation, defaults and user-visible outcomes for absent or invalid saves.
- [ ] Introduce a historical schema and migration only after basic save/resume works.
- [ ] Explain supported recovery guarantees and failure boundaries using the existing
      implementation; do not describe an ordinary file write as crash-safe.
- [ ] Save serializable progress, not entity IDs, runtime objects or resource handles.

Acceptance: a second launch restores progress; missing, corrupt, old and unsupported
future saves follow the documented policy. Fixtures use an isolated save identity
and never overwrite real user/game data. Keep inventory transaction recovery optional.

## M4 — A wave of enemies

References: [ECS](../ecs.md), [object pooling](../object-pooling.md).

- [ ] Define position, movement and health components and create a small wave.
- [ ] Update through queries; remove defeated entities through ECS lifecycle APIs.
- [ ] Explain component replacement, live identity and supported structural mutations.
- [ ] Add pooling only for a suitable transient object, such as a defeat effect,
      after demonstrating correct lifecycle without it.
- [ ] Keep wave scheduling and spawn limits explicit host policy.

Acceptance: defeated enemies stop updating, repeated resets leave no stale entities,
and the optional pooled effect resets all retained state. ECS stays authoritative;
pooling is not presented as a second enemy registry or an automatic speedup.

## M5 — A weapon that feels responsive

References: [pooling](../object-pooling.md), [tweening](../tweening.md),
[camera](../camera.md).

- [ ] Fire a projectile with explicit cadence, movement, lifetime and simple hit handling.
- [ ] Add bounded projectile reuse and a clear exhaustion policy.
- [ ] Add visual recoil, an impact effect and restrained camera feedback separately.
- [ ] Explain which effects are cosmetic and which affect simulation.
- [ ] Include reduced-motion/feedback controls and a clear reset path.

Acceptance: cadence is stable across update rates, expired projectiles are reclaimed,
reused projectiles contain no stale hit data, and one hit produces one damage result.
Use an appropriate swept test if the demonstrated speed can cross a target in one step.
Feedback settings must not change damage or hit outcomes.

## M6 — Bring a scene to life with audio

Reference: [audio](../audio.md).

- [ ] Play one effect through a cue and one music track through its intended bus.
- [ ] Add separate music/effects volume controls and demonstrate a scene transition.
- [ ] Explain source creation, reuse, concurrency limits and shutdown ownership.
- [ ] Keep generated or licensed audio small and provide a mute control.
- [ ] Distinguish host pause policy from volume and transport state.

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

- [ ] Implement patrol, detect, chase and attack with a small typed actor context.
- [ ] Explain condition evaluation, running/success/failure results and cancellation.
- [ ] Integrate the runner with entity lifecycle after the standalone behavior works.
- [ ] Keep any confirmed dynamic typing boundary inside a small documented adapter.
- [ ] Offer detection radius and cooldown experiments without adding pathfinding.

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
| M0 | Pending | |
| M1 | Pending | |
| M2 | Pending | |
| M3 | Pending | |
| M4 | Pending | |
| M5 | Pending | |
| M6 | Pending | |
| M7 | Pending | |
| M8 | Pending | |
| M9 | Pending | |
| M10 | Pending | |
| M11 | Pending | |
