---
name: tua-framework-devex
description: Build or improve Tua game code by composing its existing LOVE helpers, such as ECS, signals, stores, pools, behavior trees, sprites, camera, tweening, and audio. Use for game workflows and cross-feature DevEx, not for designing a new helper API.
---

# Tua Framework DevEx

Tua's game helpers are optional parts of normal Lua and LÖVE projects. Choose
the smallest set that improves the game's code and editing experience.

## Before Writing

- Read the relevant focused pages linked from `docs/language.md` and inspect
  matching `examples/features/src/` files. Treat those pages as the current API;
  do not invent helper names from another game framework.
- Keep native LÖVE and ordinary Lua when a helper adds no useful type,
  lifecycle, or tooling benefit. Use `.tua` annotations at public boundaries;
  let obvious local values infer their types.
- In `.tua`, use supported globals such as `ecs`, `tween`, `Camera`, `Rot`,
  `BehaviorTree`, and `AudioDesign` directly. Their imports and bundled Lua
  modules are included only when referenced. In plain `.lua`, use documented
  explicit runtime imports. Local or project-provided definitions can replace
  built-ins, so do not assume an injected global wins over a real binding.

## Compose Features Deliberately

- Establish ownership before connecting features. A pool acquisition needs a
  release path; an ECS component that stores it needs an explicit cleanup
  owner. A behavior tree can be shared while each actor keeps its own runner
  and context. Do not construct trees or audio waveforms every frame.
- Follow update order through the game loop: input, simulation, state and
  behavior transitions, tween/camera updates, then draw. Check the relevant
  feature contracts before imposing a universal order on a project.
- When wiring signals to stores or state machines, use declared event names
  and payloads across files. Keep imports and constructor return types visible
  so completion, hover, signatures, and diagnostics agree.
- Static editor previews show authored structures. The opt-in runtime inspector
  shows sampled live objects. Use both when debugging, and never infer live
  behavior from a static graph alone.

## Verify A Game Workflow

- Check and build the project, run emitted Lua or LÖVE where available, and
  inspect generated imports for used and unused helpers.
- Exercise an edit or dependency change, then test completion/hover and a
  relevant preview or diagnostic on the same fixture. Include an ownership or
  recovery case when ECS, pools, events, or generated assets interact.
- Measure frame/update allocations and editor latency before calling a helper
  performant. Preserve a small permanent regression test for a discovered
  compiler or editor defect; a temporary trial game is evidence, not the only
  test.
