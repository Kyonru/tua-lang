# RotLove and Beehive integration report

Date: 2026-11-10

## Result

Tua now has an opt-in `Rot` module with typed, independent implementations of
seeded random streams, dice, normalized colors, arena/binary-tree/sidewinder
grids, A-star and Dijkstra paths, ray-cast field of view, seeded simplex noise,
recursive string grammars, and radial light accumulation. The runtime is
emitted only when `Rot` is referenced. See [Procedural Game Helpers](../docs/roguelike.md).

This is a focused Tua-owned toolkit, not a full rotLove port. The upstream
README also lists CP437 and text displays, additional map generators, several
RNG algorithms, a 147-color catalog, and precise shadowcasting. Those pieces
are not in the new runtime. LOVE's own font and graphics APIs are a better fit
for rendering than the legacy CP437 display. Tua already has event queues,
state machines, and tweening for adjacent concerns.

## rotLove feature inventory

| Upstream area | Tua result | Notes |
| --- | --- | --- |
| Display / TextDisplay | Not ported | Legacy CP437/text cell rendering; use LOVE fonts and graphics. |
| RNG | Seeded Park–Miller stream | Independent from LOVE global RNG; no MWC/LCG/Mersenne Twister options yet. |
| StringGenerator | Implemented | Recursive `{rule}` expansion with a depth bound. |
| Maps | Partial | Arena, binary tree, and sidewinder; no Digger, Uniform, Rogue, Eller, Icey, or Divided generators. |
| Noise | Implemented | Seeded 2D simplex noise. |
| FOV | Partial | Bounded Bresenham line-of-sight; no precise shadowcasting. |
| Color | Partial | Hex and 13 basic names; no 147-entry palette/random color API. |
| Path | Implemented | Four-way unit-cost A-star and Dijkstra with a binary heap. |
| Lighting | Implemented | Radial point lights with optional opacity blocking. |
| Dice | Implemented | `dM`, `NdM`, and signed integer bonuses, using the explicit seeded RNG. |
| Scheduler / EventQueue | Not ported | Tua has neighboring state and timing helpers; no direct scheduler port was needed. |

The upstream `rotLove` license file contains notices for rotLove (MIT), rot.js
(3-clause BSD), 30log (MIT), and RandomLua (MIT). No upstream implementation
code was copied into `tuarot.lua`; future source-level reuse must preserve every
applicable notice and binary redistribution condition.

## Behavior trees and Beehive

It is possible to use Beehive from Tua now as an ordinary Lua dependency, but
its dynamic `string` statuses and callback contracts need LuaDoc declaration
files for strong checking and editor assistance. Beehive is a compact MIT
library whose functional combinators are `selector`, `sequence`, `parallel`,
`fail`, `repeat`, and `invert`. Its stateful closures require constructing a
separate tree for each actor.

Recommendation: do not add behavior-tree syntax to Tua. First validate the
generic/protocol type model using a normal module; then consider a built-in
LOVE-oriented runner with immutable reusable tree definitions and per-actor
state, explicit lifecycle/cancellation, typed context callbacks, reactive and
memory composites, frame/tick budgets, and opt-in tracing. Avoid Beehive's
ambiguous “last non-success” parallel result: expose explicit all/any/quorum
policies. The full staged plan and proposed semantics are in
[Behavior Trees For Tua and LOVE](../internal/behavior-trees-love-design.md).

The design document is a prototype plan, not a claim that the API exists. Its
first milestone requires compile, hover, completion, signature, cross-file,
and Busted probes to verify the proposed generic callback typing before any
public API is frozen.

## Verification

- `RUST_MIN_STACK=16777216 cargo test -p tua-core module_types_are_structural_and_carry_their_contracts` — passed; the bundled runtime parses and exports structural public types.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core rot_module_checks_seeded_map_and_dice_contracts` — passed; invalid map algorithms and RNG argument types are rejected.
- `RUST_MIN_STACK=16777216 cargo test -p tua-cli --test rot` — 2 passed. Builds and executes generated
  Lua, checks two maze algorithms across 20 seeds each, deterministic RNG and
  noise, pathing, visibility, color parsing, dice, grammars, lighting, and
  absence of the runtime in an unused project.
- `lua` is Lua 5.4.8 in this environment. No LuaJIT, Lua 5.1, or LOVE executable
  is installed, so native runtime compatibility was not executed here; the
  source uses Lua 5.1 syntax and the test also runs Tua's parser/checker.
- `make docs-build` — passed after using a theme icon available in the bundled
  icon set.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- `RUST_MIN_STACK=16777216 cargo test --workspace` — passed: 1,915 core tests,
  446 LSP tests, 139 CLI integration tests, all other workspace tests, and one
  ignored test. The larger test-thread stack is needed by an existing deep
  linter test; the default stack can overflow there.
- `make extension-package` — passed after allowing the tests to bind loopback;
  all 180 extension unit/profile tests passed, the source/config/theme checks
  passed, and the packaged VSIX is `editors/vscode/tua-0.7.0.vsix` (8.8 MB).
  The initial sandboxed attempt could not bind `127.0.0.1`; rerunning with
  loopback permission resolved that environment restriction.
- The VSIX was built and inspected by `vsce package`; this environment did not
  run a graphical VS Code installation/activation session or LOVE itself.

## Follow-up

1. Test the VSIX in VS Code and run the runtime under LuaJIT and LOVE when
   those are available.
2. Use an actual small roguelike to decide whether precise shadowcasting,
   weighted paths, more dungeon generators, and broader color catalogs should
   become independent follow-up features.
3. Run the behavior-tree type feasibility spike before implementing a runtime.
