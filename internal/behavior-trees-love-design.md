# Behavior trees for Tua and LOVE

Status: the core opt-in `BehaviorTree.new()` Lua runtime is implemented. This
document also records future type and tooling work; see
[Behavior Trees](../docs/behavior-trees.md) for the current API.

## Decision

Keep behavior trees as an opt-in typed runtime module. Do not add tree syntax to
Tua. The referenced [Beehive](https://github.com/drhayes/beehive.lua) API is
already expressed as ordinary Lua functions and combinators, so a native syntax
would add parser, emitter, debugger, and editor obligations without adding an
essential capability.

Tua can consume Beehive today as Lua when the project provides its source and
types. A declaration file can give existing code completion and checking. The
current library's nodes are variadic closures returning string statuses, and
stateful nodes mean each actor must receive a separately constructed tree.
Those choices are usable in Lua but leave status typos, context-shape errors,
and accidental shared state to runtime behavior.

## Using Beehive in Tua today

Yes, projects can use it now as Lua code: keep the `beehive` modules on a
project import path, require them from Tua, and include those Lua modules in
the game build. Add LuaDoc types to local leaf functions for their entity or
context, `dt`, and closed status union. To type the combinators and propagate
callback context through `sequence` or `selector`, add a Tua-readable
declaration or a small typed adapter. Until that adapter is checked against the
real library in same-file and cross-file examples, treat callback propagation
as unverified; `any`-typed external functions will not provide completion or
reject invalid statuses.

This gives projects a way to try behavior trees without changing Tua syntax or
runtime. Beehive documents that each actor needs a separately constructed tree
because its functional nodes retain state. Its `parallel` returns the last
non-success result, so a later `running` child can mask an earlier failure. A
Tua runtime should specify its parallel policy explicitly. See the upstream
[README](https://github.com/drhayes/beehive.lua).

## Recommended Tua API

The implemented `BehaviorTree` module is bundled only when referenced, like
`Camera` and `tween`. Its runtime has no LOVE globals and runs in headless
tests. The precise generic API sketched below remains future work.

The future generic type model should express:

```tua
type Status = "success" | "failure" | "running"

protocol BehaviorContext
  update: number
end

local BehaviorTree = require "tuabehavior"
local brain = BehaviorTree.sequence({
  BehaviorTree.condition(function(enemy: Enemy): boolean
    return enemy:canSee(player)
  end),
  BehaviorTree.action(function(enemy: Enemy, dt: number): BehaviorTree.Status
    return enemy:attack(dt)
  end),
})
local actor = brain:instance(enemy)
actor:update(dt)
```

This is illustrative; exact generic syntax and callback arity need a compile and
LSP prototype before the API is frozen. In particular, test generic protocols
for typed callback contexts and preserve inference through `sequence` and
`selector` before documenting that example as valid.

## Runtime semantics to settle

- A tree definition is immutable and can be shared. Every entity gets an
  instance/runner with its own cursor, child state, timers, and cancellation
  state. This avoids Beehive's requirement to rebuild stateful closure trees
  per entity.
- A tick returns a closed `Status` union. Invalid strings and nil become
  development errors with the node name and entity context.
- `sequence` and `selector` remember the currently running child. Add separate
  reactive forms that re-evaluate earlier children each tick; do not hide the
  difference behind a flag.
- Nodes support `enter`, `tick`, and `exit` lifecycle callbacks. Reset,
  cancellation, and child replacement must run exit exactly once.
- Expose wait, timeout, cooldown, repeat, invert, condition, action, and
  sub-tree nodes. Timer state belongs to the runner, never the shared node
  definition.
- Do not copy Beehive's ambiguous `parallel` behavior. Offer explicit policies
  such as all-success, any-success, and quorum, with deterministic failure and
  cancellation rules.
- Allow a per-update tick budget and maximum nesting depth so a malformed or
  expensive tree cannot freeze a LOVE frame.
- Keep debug tracing optional. A trace should identify node path, status,
  transition, elapsed time, and reset reason without allocating when disabled.

## Implementation phases

The current core runtime implements memory and reactive sequence/selector,
action, condition, wait, repeat, invert, explicit all/any parallel policies,
reset/cancel, per-actor instances, and tick/depth limits. It does not yet
propagate a concrete context type through the built-in combinators, offer
tracing or inspector integration, or provide a built-in quorum policy. These
are separate qualifications before the full design's acceptance criteria are
met. The public API and current behavior are documented in `docs/behavior-trees.md`.

1. **Type feasibility spike:** wrap or declare Beehive's real combinators and
   compile a small same-file and cross-file example using generic
   `Node<Context>`, callback inference, status unions, and protocol bounds.
   Verify hover, completion, signatures, and diagnostics. If the current type
   system cannot express the callback contract, keep the adapter explicit and
   identify the smallest missing language capability before proposing syntax.
2. **Core runner:** implement sequence, selector, condition, action, wait,
   repeat, invert, cancellation, reset, and independent per-actor instances.
   Add Busted tests for deterministic tick traces and cleanup semantics.
3. **LOVE helpers:** add optional `dt` integration, entity-oriented examples,
   and an inspector-friendly tree trace. Keep rendering and LOVE globals out of
   the runtime core.
4. **Qualification:** test many entities sharing one definition, nested
   cross-file nodes, callback errors, frame budgets, hot reload of definitions,
   and headless execution. Document memory and scheduling costs.

## Acceptance criteria

- The same immutable tree definition can create independent runners for many
  entities without state leaking between them.
- All node callbacks receive the concrete context type across module boundaries.
- Only the three declared statuses can leave a node; diagnostics identify
  invalid callback results.
- Reset/cancel/failure/success lifecycle order is deterministic and tested.
- Reactive and memory composites have separate names and documented semantics.
- Tick and recursion limits are configurable and tested.
- The runtime is demand-loaded, covered by Busted and compiler tests, and has a
  runnable LOVE example plus a headless example.

## Reference and license notes

Beehive is a small MIT-licensed functional behavior-tree library. Its model is
useful as an API reference, but Tua should independently implement the runner
and retain its own node semantics. Beehive documents `selector`, `sequence`,
`parallel`, `fail`, `repeat`, and `invert`; the original interface uses string
statuses and stateful tree closures. See its
[README](https://github.com/drhayes/beehive.lua) and
[license](https://github.com/drhayes/beehive.lua/blob/main/LICENSE).

Tua already has typed classes, protocols, and generics. Those are sufficient
building blocks for an initial library API. New language syntax should be
considered only if real behavior-tree projects show that ordinary constructors
cannot express or inspect common trees clearly.
