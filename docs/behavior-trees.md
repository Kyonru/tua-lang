---
icon: lucide/git-branch
---

# Behavior trees

`BehaviorTree` is an optional Lua runtime for choosing an actor's behavior each
frame. In Tua, calling `BehaviorTree.new()` inserts the runtime import and
copies `tuabehavior.lua` into the build. Projects that never reference it do
not include it. Plain Lua can use `local BehaviorTree = require("tuabehavior")`.

```tua
local enemy = { seesPlayer = false, shots = 0 }
local tree = BehaviorTree.new(BehaviorTree.selector({
  BehaviorTree.sequence({
    BehaviorTree.condition(function(actor) return actor.seesPlayer end),
    BehaviorTree.action(function(actor, dt)
      actor.shots = actor.shots + 1
      return "success"
    end),
  }),
  BehaviorTree.action(function(actor) return "running" end),
}))
local runner = tree:instance(enemy)

function love.update(dt)
  runner:update(dt)
end
```

`new()` may also be called without a root; call `tree:setRoot(node)` before
creating a runner. A definition can create many runners. Each runner owns its
own timer and composite cursor, so actors do not share execution state. Keep
the definition's nodes unchanged after instancing.

## Static editor preview

In VS Code, run **Tua: Preview Behavior Tree** from a `.tua` file to see the
hierarchy of top-level `local tree = BehaviorTree.new(...)` definitions. Direct
nested constructor calls appear as nodes with source links. A top-level
`tree:setRoot(...)` call also supplies the root for a tree created without one.
Hover a tree name to open that tree directly. The preview places child nodes
below their parents; scroll sideways to explore wide trees. Click a node to
jump to its constructor. The preview refreshes after edits and
uses the same opt-in `tua.preview.enabled` setting as the state-machine preview.
Open the [feature example](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/behavior_tree.tua)
to try the preview on a complete tree.

The preview reads source only; it never runs game code. Nodes supplied by a
variable, helper function, or another module appear as **Computed node** because
their structure cannot be known from the local expression. It does not show
live runner statuses.

`action(callback, enter?, exit?)` callbacks receive `(context, dt)` and return
`"success"`, `"failure"`, or `"running"`. `enter(context)` runs on first visit;
`exit(context, reason)` runs when the action finishes or its runner is reset
or cancelled. Conditions return a boolean. `wait(seconds)` keeps elapsed time
per runner. `sequence` and `selector` remember a running child. `invert` swaps
success and failure; `repeatN(count, child)` repeats a successful child across
updates and fails if the child fails.

`reactiveSequence` and `reactiveSelector` revisit earlier children every
update and interrupt a previously running child when a higher-priority branch
changes the outcome. `parallelAll` succeeds when every child succeeds and
fails when any child fails. `parallelAny` succeeds when any child succeeds and
fails when all children fail. Both keep completed child results until the
parallel node completes, then cancel any still-running siblings.

Use `runner:reset()` to clear running state or `runner:cancel()` when an actor
is removed. Invalid action results, recursive updates, negative `dt`, shared
node objects, and excessive traversal depth or work raise errors. The default
limits are 64 levels and 1,000 node visits per update; `runner.maxDepth` and
`runner.maxSteps` can be lowered for a game. The runtime has no LOVE dependency
and can be exercised in headless tests.

Current callback declarations accept a dynamic context. Annotate callback
parameters in Tua to get member checking within each leaf. Generic context
propagation through built-in combinators remains a follow-up type-system task.
